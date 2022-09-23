use std::sync::Arc;

use ash::vk;
use turbosloth::LazyCache;

use raven_rhi::{RHI, backend::{Device, barrier::{self, ImageBarrier}, Swapchain}, pipeline_cache::PipelineCache};

use crate::{compiled_graph::CompiledRenderGraph, transient_resource_cache::TransientResourceCache};
use crate::graph_builder::{RenderGraphBuilder, TemporaryResourceRegistry, ExportedTemporalResources, TemporaryResourceState};

enum RenderGraphTemporalResources {
    Inert(TemporaryResourceRegistry),
    Exported(ExportedTemporalResources),
}

impl Default for RenderGraphTemporalResources {
    fn default() -> Self {
        RenderGraphTemporalResources::Inert(Default::default())
    }
}

/// Render graph executor to build and run a render graph with RHI.
pub struct Executor {
    device: Arc<Device>,

    compiled_rg: Option<CompiledRenderGraph>,
    pipeline_cache: PipelineCache,

    transient_resource_cache: TransientResourceCache,
    temporal_resources: RenderGraphTemporalResources,
}

impl Executor {
    pub fn new(rhi: &RHI) -> anyhow::Result<Self> {
        Ok(Self {
            device: rhi.device.clone(),

            compiled_rg: None,
            pipeline_cache: PipelineCache::new(LazyCache::create()),

            transient_resource_cache: TransientResourceCache::new(),
            temporal_resources: Default::default(),
        })
    }

    pub fn prepare<PrepareFunc>(
        &mut self,
        prepare_func: PrepareFunc,
    ) -> anyhow::Result<()>
    where
        PrepareFunc: FnOnce(&mut RenderGraphBuilder)
    {
        let mut rg_builder = RenderGraphBuilder::new(
            self.device.clone(),
            // fill the last frame's temporal resources, so that this frame can reuse some of the temporal resources
            match &self.temporal_resources {
                RenderGraphTemporalResources::Inert(resources) => resources.clone_assuming_inert(),
                RenderGraphTemporalResources::Exported(_) => {
                    panic!("prepare() was called when the render graph is active!")
                }
            }
        );

        // user-side callback to build the render graph with custom passes
        prepare_func(&mut rg_builder);
        // now the render graph is ready to compile and run
        
        let (rg, exported_temp_resources) = rg_builder.export_all_imported_resources();

        // analyzed all passes and register pipelines to pipeline cache
        self.compiled_rg = Some(rg.compile(&mut self.pipeline_cache));

        // update and compile pipeline shaders
        match self.pipeline_cache.prepare() {
            Ok(()) => {
                // If this frame is successfully prepared, we get all the resources ready to be drawn
                self.temporal_resources = RenderGraphTemporalResources::Exported(exported_temp_resources);

                // create new pipelines
                self.pipeline_cache.update_pipelines(&self.device);

                Ok(())
            },
            Err(err) => {
                glog::warn!("Failed to prepare render graph!");
                // some shaders may failed in compilation, but after the compile() is called, some resources are created and ready.
                // we can reuse these resources in the next attempt.
                // just changed the Imported and Exported resources to Inert and stick them into temporal resources.

                let temporal_resources = match &mut self.temporal_resources {
                    RenderGraphTemporalResources::Inert(resources) => resources,
                    RenderGraphTemporalResources::Exported(_) => unreachable!(),
                };

                for (k, v) in exported_temp_resources.0.0 {
                    // this is a new resource of this frame
                    #[allow(clippy::map_entry)]
                    if !temporal_resources.0.contains_key(&k) {
                        let resource = match v {
                            res @ TemporaryResourceState::Inert { .. } => res,
                            TemporaryResourceState::Imported { resource, .. }
                            | TemporaryResourceState::Exported { resource, .. } => {
                                TemporaryResourceState::Inert {
                                    resource,
                                    access: vk_sync::AccessType::Nothing,
                                }
                            }
                        };

                        temporal_resources.0.insert(k, resource);
                    }
                }

                Err(err)
            }
        }
    }

    pub fn draw(&mut self, swapchain: &mut Swapchain) {
        let compiled_rg = if let Some(rg) = self.compiled_rg.take() {
            rg  
        } else {
            glog::warn!("Render Graph is not compiled yet, draw request denied!");
            return;
        };

        // wait for all the command buffers in this frame to be submitted, then we can record the new commands
        let draw_frame = self.device.begin_frame();
        let device = &self.device;

        // reset and begin recording commands
        for cb in [
            &draw_frame.main_command_buffer,
            &draw_frame.present_command_buffer
        ] {
            unsafe {
                device.raw
                    .reset_command_buffer(cb.raw, vk::CommandBufferResetFlags::default())
                    .unwrap();
            
                device.raw
                    .begin_command_buffer(cb.raw, 
                        &vk::CommandBufferBeginInfo::builder()
                            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                            .build())
                    .unwrap();  
            }
        }

        let mut executing_rg;
        // record and submit main command buffers
        // TODO: dispatch some compute commands to async compute queue
        {
            let main_cb = &draw_frame.main_command_buffer;

            // create or import the actual resources into render graph.
            executing_rg = compiled_rg.prepare_execute(&device, &mut self.pipeline_cache, &mut self.transient_resource_cache);

            executing_rg.record_commands(&main_cb);

            unsafe {
                device.raw.end_command_buffer(main_cb.raw).unwrap();
            }

            let submit_info = [vk::SubmitInfo::builder()
                .command_buffers(&[main_cb.raw])
                .build()];

            unsafe {
                device.raw
                    .reset_fences(std::slice::from_ref(&main_cb.submit_done_fence))
                    .expect("Failed to reset command buffer submit fence!");

                device.raw
                    .queue_submit(device.global_queue.raw, &submit_info, main_cb.submit_done_fence)
                    .expect("Failed to submit main commands to global queue!");
            }
        }
        // after this point, GPU is busying submitting basic commands and executing
        // we acquired the image as late as possible, because it can be blocked (i.e. the rendering is not complete)
        let swapchain_image = swapchain.acquire_next_image().expect("Failed to acquire next image!");

        // then submit the present command
        let finished_rg = {
            let present_cb = &draw_frame.present_command_buffer;

            // manually transition swapchain image to ComputeShaderWrite.
            // we didn't create image view for the swapchain image, we just copy the frame we want to present to swapchain image.
            // so we don't need image view for the framebuffer.
            barrier::image_barrier(&device, &present_cb, &[
                ImageBarrier::builder()
                    .image(&swapchain_image.image)
                    .prev_access(std::slice::from_ref(&vk_sync::AccessType::Present))
                    .next_access(std::slice::from_ref(&vk_sync::AccessType::ComputeShaderWrite))
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    // set this to true will force prev_access set to vk::ImageLayout::UNDEFINED
                    // since we discard the contents, we don't care what previous access type it is
                    .discard_contents(true) 
                    .build().unwrap()
                ]
            );

            let finished_rg = executing_rg.record_present_commands(&present_cb, swapchain_image.image.clone());

            // back to present
            barrier::image_barrier(&device, &present_cb, &[
                ImageBarrier::builder()
                    .image(&swapchain_image.image)
                    .prev_access(std::slice::from_ref(&vk_sync::AccessType::ComputeShaderWrite))
                    .next_access(std::slice::from_ref(&vk_sync::AccessType::Present))
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .build().unwrap()
                ]
            );

            unsafe {
                device.raw.end_command_buffer(present_cb.raw).unwrap();
            }

            let submit_info = [vk::SubmitInfo::builder()
                // wait until compute shader finished writing
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COMPUTE_SHADER])
                .wait_semaphores(&[swapchain_image.acquire_semaphore])
                .signal_semaphores(&[swapchain_image.render_finished_semaphore])
                .command_buffers(&[present_cb.raw])
                .build()];
            
            // reset fence and submit
            unsafe {
                device.raw
                    .reset_fences(std::slice::from_ref(&present_cb.submit_done_fence))
                    .expect("Failed to reset command buffer submit fence!");

                device.raw
                    .queue_submit(device.global_queue.raw, &submit_info, present_cb.submit_done_fence)
                    .expect("Failed to submit present commands to global queue!");
            }

            finished_rg
        };

        // present this frame
        swapchain.present(swapchain_image);

        // render graph completes its mission, time to throw it away
        // change all temporal resources back to Inert and be ready to next frame
        self.temporal_resources = match std::mem::take(&mut self.temporal_resources) {
            RenderGraphTemporalResources::Inert(_) => {
                panic!("Temporal resources are in Inert state, did you forget to call prepare()?");
            }
            RenderGraphTemporalResources::Exported(res) => {
                RenderGraphTemporalResources::Inert(res.consume(&finished_rg))
            }
        };

        // store all transient resources back to the cache
        finished_rg.release_owned_resources(&mut self.transient_resource_cache);

        // take this frame back, we want to keep only one owner when we start a new frame (see begin_frame())
        self.device.end_frame(draw_frame);
    }
}
