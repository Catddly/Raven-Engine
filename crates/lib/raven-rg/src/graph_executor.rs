use std::sync::Arc;

use ash::vk;

use raven_scene::camera::CameraFrameConstants;
use raven_rhi::{Rhi, backend::{Device, barrier::{self, ImageBarrier}, Swapchain}, pipeline_cache::PipelineCache, dynamic_buffer::DynamicBuffer, global_constants_descriptor};

use crate::{compiled_graph::CompiledRenderGraph, transient_resource_cache::TransientResourceCache, graph_builder::TemporalResource};
use crate::graph_builder::{RenderGraphBuilder, TemporalResourceRegistry, ExportedTemporalResources, TemporalResourceState};

enum RenderGraphTemporalResources {
    Inert(TemporalResourceRegistry),
    Exported(ExportedTemporalResources),
}

impl RenderGraphTemporalResources {
    pub fn clean(self, device: &Device) {
        let registry = match self {
            Self::Inert(registry) => {
                registry
            },
            Self::Exported(resources) => {
                resources.0
            }
        };

        for (_, resource) in registry.0 {
            match resource {
                TemporalResourceState::Inert { resource, .. } => {
                    match resource {
                        TemporalResource::Image(img) => {
                            let img = Arc::try_unwrap(img)
                                .expect("Render graph temporal image resource may not be retained!");
                            
                            device.destroy_image(img);
                        }
                        TemporalResource::Buffer(buf) => {
                            let buf = Arc::try_unwrap(buf)
                                .expect("Render graph temporal buffer resource may not be retained!");
                            
                            device.destroy_buffer(buf);
                        }
                    }
                }
                TemporalResourceState::Imported { .. } => {
                    panic!("Invalid render graph temporal resource state while cleaning!");                     
                }
                TemporalResourceState::Exported { .. } => {
                    panic!("Invalid render graph temporal resource state while cleaning!");                     
                }
            }
        }
    }
}

impl Default for RenderGraphTemporalResources {
    fn default() -> Self {
        RenderGraphTemporalResources::Inert(Default::default())
    }
}

/// Render graph executor to build and run a render graph with RHI.
pub struct GraphExecutor {
    device: Arc<Device>,

    compiled_rg: Option<CompiledRenderGraph>,
    pipeline_cache: PipelineCache,

    transient_resource_cache: TransientResourceCache,
    temporal_resources: RenderGraphTemporalResources,

    global_dynamic_buffer: DynamicBuffer,
    global_dynamic_constants_set: vk::DescriptorSet,
}

pub struct ExecutionParams<'a> {
    pub device: &'a Device,
    pub pipeline_cache: &'a mut PipelineCache,
    pub global_constants_set: vk::DescriptorSet,
    pub draw_frame_context_layout: DrawFrameContextLayout,
}

// TODO: temporary
#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct LightFrameConstants {
    pub color    : [f32; 3], // color in range [0.0, 1.0]
    pub shadowed : u32,      // it is a bool
    pub direction: [f32; 3], // direction vector
    pub intensity: f32,
}

impl Default for LightFrameConstants {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            shadowed: 0, // false
            direction: [1.0, 0.0, 0.0],
            intensity: 1.0
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct FrameConstants {
    pub cam_matrices: CameraFrameConstants,
    pub light_constants: [LightFrameConstants; 10],

    pub frame_index: u32,
    pub pre_exposure_mult: f32,
    pub pre_exposure_prev_frame_mult: f32,
    pub pre_exposure_delta: f32,

    pub directional_light_count: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub pad2: u32,
}

#[derive(Copy, Clone)]
pub struct DrawFrameContextLayout {
    pub frame_constants_offset: u32,
}

impl GraphExecutor {
    pub fn new(rhi: &Rhi) -> anyhow::Result<Self> {
        let global_dynamic_buffer = DynamicBuffer::new(rhi);
        let global_dynamic_constants_set = global_constants_descriptor::create_engine_global_constants_descriptor_set(rhi, &global_dynamic_buffer);

        Ok(Self {
            device: rhi.device.clone(),

            compiled_rg: None,
            pipeline_cache: PipelineCache::new(),

            transient_resource_cache: TransientResourceCache::new(),
            temporal_resources: Default::default(),

            global_dynamic_buffer,
            global_dynamic_constants_set,
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
        // temporal resources only used when preparing the render graph
        prepare_func(&mut rg_builder);

        // now the render graph is ready to compile and run
        let (rg, exported_temporal_resources) = rg_builder.build();

        // analyzed all passes and register pipelines to pipeline cache
        self.compiled_rg = Some(rg.compile(&mut self.pipeline_cache));

        // update and compile pipeline shaders
        match self.pipeline_cache.prepare(&self.device) {
            Ok(()) => {
                // if this frame is successfully prepared, we have got all the resources ready to be drawn
                self.temporal_resources = RenderGraphTemporalResources::Exported(exported_temporal_resources);

                // create new pipelines and destroy stale pipelines
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

                for (k, v) in exported_temporal_resources.0.0 {
                    // change all temporal resources into inert state and  insert it back
                    #[allow(clippy::map_entry)]
                    if !temporal_resources.0.contains_key(&k) {
                        let resource = match v {
                            res @ TemporalResourceState::Inert { .. } => res,
                            TemporalResourceState::Imported { resource, .. }
                            | TemporalResourceState::Exported { resource, .. } => {
                                TemporalResourceState::Inert {
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

    pub fn draw(&mut self, draw_frame_context: &FrameConstants, swapchain: &mut Swapchain) {
        // begin drawing (record commands and submit)
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

        // update frame constant to global dynamic buffer
        let frame_constants_offset = self.global_dynamic_buffer.push(draw_frame_context);

        let frame_constants_layout = DrawFrameContextLayout {
            frame_constants_offset,
        };

        let mut executing_rg;
        // record and submit main command buffers
        // TODO: dispatch some compute commands to async compute queue
        {
            let main_cb = &draw_frame.main_command_buffer;

            // create or import the actual resources into render graph.
            executing_rg = compiled_rg.prepare_execute(ExecutionParams {
                    device: &self.device,
                    pipeline_cache: &mut self.pipeline_cache,
                    global_constants_set: self.global_dynamic_constants_set,
                    draw_frame_context_layout: frame_constants_layout,
                },
                &mut self.transient_resource_cache,
                &mut self.global_dynamic_buffer,
            );

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
            barrier::image_barrier(&device, present_cb.raw, &[
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

            let retired_rg = executing_rg.record_present_commands(&present_cb, swapchain_image.image.clone());

            // back to present
            barrier::image_barrier(&device, present_cb.raw, &[
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

            retired_rg
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

        self.global_dynamic_buffer.advance_frame();
        // take this frame back, we want to keep only one owner when we start a new frame (see begin_frame())
        self.device.end_frame(draw_frame);
    }

    /// Explicitly clean up all the resources using inside a render graph.
    pub fn shutdown(self) {
        self.device.wait_idle();

        self.global_dynamic_buffer.clean(&self.device);
        self.transient_resource_cache.clean(&self.device);
        self.temporal_resources.clean(&self.device);
        self.pipeline_cache.clean(&self.device);
    }
}
