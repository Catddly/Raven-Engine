use std::{sync::Arc, collections::HashMap};

// TODO: remove this (render graph should not directly contain eny graphic API)
use ash::vk;
use arrayvec::ArrayVec;

use raven_core::container::TempList;
use raven_rhi::{
    backend::{Device, ImageViewDesc, Image, Buffer, RasterPipeline, ComputePipeline, CommandBuffer, RenderPass},
    backend::constants,
    backend::RHIError,
    backend::renderpass::FrameBufferCacheKey,
    backend::pipeline::CommonPipeline,
    backend::descriptor::DescriptorSetBinding,
    dynamic_buffer::DynamicBuffer,
};

use crate::executor::{ExecutionParams};

use super::{
    resource::{SRV, UAV, RT},
    compiled_graph::{RenderGraphPipelineHandles, RegisteredResource, GraphPreparedResourceRef},
    graph_resource::{GraphResourceHandle, GraphResourceRef, GraphRasterPipelineHandle, GraphComputePipelineHandle},
};

pub struct PassImageBinding {
    handle: GraphResourceHandle,
    view_desc: ImageViewDesc,
    layout: vk::ImageLayout,
}

pub struct PassBufferBinding {
    handle: GraphResourceHandle,
}

pub enum RenderGraphPassBinding {
    Image(PassImageBinding),
    ImageArray(Vec<PassImageBinding>),
    Buffer(PassBufferBinding),

    DynamicBuffer(u32),
    DynamicStorageBuffer(u32),
}

impl RenderGraphPassBinding {
    pub fn with_aspect(&mut self, aspect: vk::ImageAspectFlags) {
        match self {
            RenderGraphPassBinding::Image(image) => {
                image.view_desc.aspect_mask = aspect;
            },
            RenderGraphPassBinding::ImageArray(images) => {
                for image in images {
                    image.view_desc.aspect_mask = aspect;
                }
            },
            _ => panic!("Try to add ImageAspectFlags to buffers!"),
        }
    }
}

pub trait RenderGraphPassBindable {
    fn bind(&self) -> RenderGraphPassBinding;
}

impl RenderGraphPassBindable for GraphResourceRef<Image, SRV> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Image(PassImageBinding {
            handle: self.handle.clone(),
            view_desc: ImageViewDesc::default(),
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        })
    }
}

impl RenderGraphPassBindable for Vec<GraphResourceRef<Image, SRV>> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::ImageArray(self.iter()
            .map(|refer| {
                PassImageBinding {
                    handle: refer.handle.clone(),
                    view_desc: ImageViewDesc::default(),
                    layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                }
            })    
            .collect()
        )
    }
}

impl RenderGraphPassBindable for GraphResourceRef<Image, UAV> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Image(PassImageBinding {
            handle: self.handle.clone(),
            view_desc: ImageViewDesc::default(),
            layout: vk::ImageLayout::GENERAL,
        })
    }
}

impl RenderGraphPassBindable for Vec<GraphResourceRef<Image, UAV>> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::ImageArray(self.iter()
            .map(|refer| {
                PassImageBinding {
                    handle: refer.handle.clone(),
                    view_desc: ImageViewDesc::default(),
                    layout: vk::ImageLayout::GENERAL,
                }
            })    
            .collect()
        )
    }
}

#[derive(Default)]
pub struct PipelineSetLayoutBindings<'a> {
    /// Pipeline will create descriptor set for user, and release the descriptor in the next frame.
    bindings: ArrayVec<(u32, &'a [RenderGraphPassBinding]), { constants::MAX_DESCRIPTOR_SET_COUNT }>,
}

pub struct PipelineBindings<'a, HandleType> {
    pipeline_handle: HandleType,
    pub(crate) set_layouts: PipelineSetLayoutBindings<'a>,
    pub(crate) raw_sets: HashMap<u32, vk::DescriptorSet>,
}

impl<'a, HandleType> PipelineBindings<'a, HandleType> {
    pub fn new(pipeline_handle: HandleType) -> Self {
        Self {
            pipeline_handle,
            set_layouts: Default::default(),
            raw_sets: Default::default(),
        }
    }

    pub fn descriptor_set(mut self, set_idx: u32, bindings: &'a [RenderGraphPassBinding]) -> Self {
        self.set_layouts.bindings.push((set_idx, bindings));
        self
    }

    // TODO: may be we want engine to manager the raw global descriptor set
    pub fn raw_descriptor_set(mut self, set_idx: u32, set: vk::DescriptorSet) -> Self {
        self.raw_sets.insert(set_idx, set);
        self
    }
}

pub trait IntoPipelineDescriptorBindings : Sized {
    fn into_bindings<'a>(self) -> PipelineBindings<'a, Self>;
}

impl IntoPipelineDescriptorBindings for GraphRasterPipelineHandle {
    fn into_bindings<'a>(self) -> PipelineBindings<'a, Self> {
        PipelineBindings::new(self)
    }
}

impl IntoPipelineDescriptorBindings for GraphComputePipelineHandle {
    fn into_bindings<'a>(self) -> PipelineBindings<'a, Self> {
        PipelineBindings::new(self)
    }
}

pub struct BoundComputePipeline<'context, 'exec, 'a> {
    context: &'context PassContext<'exec, 'a>,
    pipeline: Arc<ComputePipeline>,
}

impl<'context, 'exec, 'a> BoundComputePipeline<'context, 'exec, 'a> {
    pub fn dispatch(
        &self,
        threads: [u32; 3],
    ) {
        let device = self.context.context.execution_params.device;
        let dispatch_groups = self.pipeline.dispatch_groups;

        unsafe {
            device.raw.cmd_dispatch(
                self.context.cb.raw,
                // divide floor
                threads[0] / dispatch_groups[0],
                threads[1] / dispatch_groups[1],
                threads[2] / dispatch_groups[2]
            );
        }
    }

    pub fn push_constants(
        &self,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        bytes: &[u8],
    ) {
        let device = self.context.context.execution_params.device;

        unsafe {
            device.raw.cmd_push_constants(
                self.context.cb.raw, 
                self.pipeline.pipeline.pipeline_layout,
                stage_flags, 
                offset, 
                bytes
            );
        }
    }
}

pub struct BoundRasterPipeline<'context, 'exec, 'a> {
    context: &'context PassContext<'exec, 'a>,
    pipeline: Arc<RasterPipeline>,
}

impl<'context, 'exec, 'a> BoundRasterPipeline<'context, 'exec, 'a> {
    pub fn push_constants(
        &self,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        bytes: &[u8],
    ) {
        let device = self.context.context.execution_params.device;

        unsafe {
            device.raw.cmd_push_constants(
                self.context.cb.raw, 
                self.pipeline.pipeline_layout, 
                stage_flags, 
                offset, 
                bytes
            );
        }
    }
}

pub struct ExecuteContext<'exec, 'a> {
    pub execution_params: &'a ExecutionParams<'exec>,

    pub(crate) pipelines: &'a RenderGraphPipelineHandles,
    pub(crate) registered_resources: &'a Vec<RegisteredResource>,
    pub(crate) global_dynamic_buffer: &'a mut DynamicBuffer,
}

impl<'exec, 'a> ExecuteContext<'exec, 'a> {
    pub(crate) fn get_image_view(&self, handle: GraphResourceHandle, view_desc: &ImageViewDesc) -> anyhow::Result<vk::ImageView, RHIError> {
        let image = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Image(image) => image,
            _ => panic!("Expect image, but pass in a non-image graph resource handle!"),
        };

        let device = self.execution_params.device;
        image.view(device, &view_desc)
    }

    pub(crate) fn get_image(&self, handle: GraphResourceHandle) -> &Image {
        let image = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Image(image) => image,
            _ => panic!("Expect image, but pass in a non-image graph resource handle!"),
        };

        image
    }

    pub(crate) fn get_buffer(&self, handle: GraphResourceHandle) -> &Buffer {
        let buffer = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Buffer(buffer) => buffer,
            _ => panic!("Expect buffer, but pass in a non-buffer graph resource handle!"),
        };

        buffer
    }

    pub(crate) fn get_raster_pipeline(&self, handle: GraphRasterPipelineHandle) -> Arc<RasterPipeline> {
        let pipeline = self.pipelines.raster_pipeline_handles[handle.idx];
        self.execution_params.pipeline_cache.get_raster_pipeline(pipeline)
    }

    pub(crate) fn get_compute_pipeline(&self, handle: GraphComputePipelineHandle) -> Arc<ComputePipeline> {
        let pipeline = self.pipelines.compute_pipeline_handles[handle.idx];
        self.execution_params.pipeline_cache.get_compute_pipeline(pipeline)
    }
}

/// Render pass context to give user to do custom command buffer recording ability and etc.
pub struct PassContext<'exec, 'a> {
    /// Command Buffer to record rendering commands to.
    pub cb: &'a CommandBuffer,
    /// Context Relative Resources to be used inside this render pass. 
    pub context: ExecuteContext<'exec, 'a>,
}

impl<'exec, 'a> PassContext<'exec, 'a> {
    #[inline]
    pub fn device(&self) -> &Device {
        &self.context.execution_params.device
    }

    pub fn global_dynamic_buffer(&mut self) -> &mut DynamicBuffer {
        self.context.global_dynamic_buffer
    } 

    pub fn begin_render_pass(
        &mut self,
        render_pass: &RenderPass,
        extent: [u32; 2],
        color_attachments: &[(GraphResourceRef<Image, RT>, &ImageViewDesc)],
        depth_attachment: Option<(GraphResourceRef<Image, RT>, &ImageViewDesc)>,
    ) -> anyhow::Result<(), RHIError> {
        let device = self.context.execution_params.device;

        // get or create the framebuffer from the cache
        let framebuffer = render_pass.frame_buffer_cache.get_or_create(device, FrameBufferCacheKey::new(
            extent, 
            color_attachments.iter().map(|(refer, _)| {
                // TODO: is this verbose?
                //&refer.desc
                &self.context.get_image(refer.handle).desc
            }), 
            depth_attachment.as_ref().map(|(refer, _)| {
                //&refer.desc
                &self.context.get_image(refer.handle).desc
            })
        ));

        // collect all image views
        let attachments = color_attachments.iter()
            .chain(depth_attachment.as_ref().into_iter())
            .map(|(refer, view)| self.context.get_image_view(refer.handle, &view))
            .collect::<anyhow::Result<ArrayVec<vk::ImageView, { constants::MAX_RENDERPASS_ATTACHMENTS + 1 }>, RHIError>>();
        let attachments = attachments?;

        // fill in the image view for bindless framebuffer
        let mut render_pass_attachments = vk::RenderPassAttachmentBeginInfoKHR::builder()
            .attachments(&attachments)
            .build();

        let renderpass_begin_info = vk::RenderPassBeginInfo::builder()
            .push_next(&mut render_pass_attachments)
            .render_pass(render_pass.raw)
            .render_area(vk::Rect2D {
                extent: vk::Extent2D {
                    width: extent[0],
                    height: extent[1],
                },
                offset: vk::Offset2D {
                    x: 0, y: 0,
                },
            })
            .framebuffer(framebuffer)
            .build();

        unsafe {
            device.raw.cmd_begin_render_pass(
                self.cb.raw, 
                &renderpass_begin_info, 
                vk::SubpassContents::INLINE
            );
        }

        Ok(())
    }

    #[inline]
    pub fn end_render_pass(
        &mut self,
    ) {
        unsafe {
            self.device().raw.cmd_end_render_pass(self.cb.raw);
        }
    }

    #[inline]
    pub fn set_default_viewport_and_scissor(&self, [width, height]: [u32; 2]) {
        self.set_viewport([width, height]);
        self.set_scissor([width, height]);
    }

    #[inline]
    pub fn set_viewport(&self, [width, height]: [u32; 2]) {
        unsafe {
            self.device().raw.cmd_set_viewport(
                self.cb.raw, 
                0,
                // negative height of viewport to flip vulkan y NDC coordinates
                &[vk::Viewport {
                    x: 0.0, y: (height as f32),
                    width: width as f32, 
                    height: -(height as f32),
                    min_depth: 0.0,
                    max_depth: 1.0,
                }]
            );
        }
    }

    #[inline]
    pub fn set_scissor(&self, [width, height]: [u32; 2]) {
        unsafe {
            self.device().raw.cmd_set_scissor(
                self.cb.raw,
                0,
                &[
                    vk::Rect2D {
                        offset: vk::Offset2D {
                            x: 0, y: 0
                        },
                        extent: vk::Extent2D {
                            width,
                            height,
                        },
                    }
                ]
            );
        }
    }

    pub fn bind_raster_pipeline(&self, bindings: PipelineBindings<'_, GraphRasterPipelineHandle>) -> anyhow::Result<BoundRasterPipeline, RHIError> {
        let pipeline = self.context.get_raster_pipeline(bindings.pipeline_handle);
        self.bind_pipeline(self.context.execution_params.device, pipeline.as_ref(), &bindings.set_layouts, &bindings.raw_sets)?;

        Ok(BoundRasterPipeline {
            context: self,
            pipeline,
        })
    }

    pub fn bind_compute_pipeline(&self, bindings: PipelineBindings<'_, GraphComputePipelineHandle>) -> anyhow::Result<BoundComputePipeline, RHIError> {
        let pipeline = self.context.get_compute_pipeline(bindings.pipeline_handle);
        self.bind_pipeline(self.context.execution_params.device, pipeline.as_ref(), &bindings.set_layouts, &bindings.raw_sets)?;

        Ok(BoundComputePipeline {
            context: self,
            pipeline,
        })
    }

    /// bind pipeline and pipeline's descriptors
    fn bind_pipeline(
        &self,
        device: &Device,
        pipeline: &CommonPipeline,
        set_layout: &PipelineSetLayoutBindings,
        raw_sets: &HashMap<u32, vk::DescriptorSet>,
    ) -> anyhow::Result<(), RHIError> {
        // bind pipeline
        unsafe {
            device.raw
                .cmd_bind_pipeline(self.cb.raw, pipeline.pipeline_bind_point, pipeline.pipeline);
        }
        
        // bind engine global frame constants
        // TODO: do we really need to bind it every time bound pipeline?
        if pipeline.set_layout_infos.get(2).is_some() {
            unsafe {
                device.raw.cmd_bind_descriptor_sets(
                    self.cb.raw, 
                    pipeline.pipeline_bind_point, 
                    pipeline.pipeline_layout,
                    2,
                    &[self.context.execution_params.global_constants_set], 
                    &[
                        // binding 0
                        self.context.execution_params.draw_frame_context_layout.frame_constants_offset
                    ]
                );
            }
        }

        // create and bind pipeline's descriptor sets
        for (set_idx, bindings) in set_layout.bindings.iter() {
            // trying to bind a resource that is not defined in pipeline's shader
            if pipeline.set_layout_infos.get(*set_idx as usize).is_none() {
                continue;
            }

            let bindings: anyhow::Result<Vec<_>, RHIError> = bindings.iter()
                .map(|pass_bingding| {
                    Ok(match &pass_bingding {
                        RenderGraphPassBinding::Image(image) => DescriptorSetBinding::Image(vk::DescriptorImageInfo::builder()
                            .image_layout(image.layout)
                            .image_view(self.context.get_image_view(image.handle, &image.view_desc)?)
                            .build()
                        ),
                        RenderGraphPassBinding::ImageArray(images) => DescriptorSetBinding::ImageArray(
                                images.iter()
                                .map(|image| {
                                    Ok(vk::DescriptorImageInfo::builder()
                                        .image_layout(image.layout)
                                        .image_view(self.context.get_image_view(image.handle, &image.view_desc)?)
                                        .build())
                                })
                                .collect::<anyhow::Result<Vec<_>, RHIError>>()?
                        ),
                        RenderGraphPassBinding::Buffer(buffer) => DescriptorSetBinding::Buffer(vk::DescriptorBufferInfo::builder()
                            .buffer(self.context.get_buffer(buffer.handle).raw)
                            .range(vk::WHOLE_SIZE)
                            .build()
                        ),
                        RenderGraphPassBinding::DynamicBuffer(offset) => DescriptorSetBinding::DynamicBuffer {
                            buffer_info: vk::DescriptorBufferInfo::builder()
                                .buffer(self.context.global_dynamic_buffer.buffer.raw)
                                .range(self.context.global_dynamic_buffer.max_uniform_buffer_range() as _)
                                .build(),
                            offset: *offset,
                        },
                        RenderGraphPassBinding::DynamicStorageBuffer(offset) => DescriptorSetBinding::DynamicStorageBuffer {
                            buffer_info: vk::DescriptorBufferInfo::builder()
                                .buffer(self.context.global_dynamic_buffer.buffer.raw)
                                //.range(self.context.global_dynamic_buffer.max_storage_buffer_range() as _)
                                .range(vk::WHOLE_SIZE)
                                .build(),
                            offset: *offset,
                        }
                    })
                })
                .collect();
            let bindings = bindings?;

            self.bind_descriptor_set(&pipeline, *set_idx, &bindings);
        }

        let device = self.context.execution_params.device;
        // TODO: unsafe. user specific descriptor set index may collide with raw descriptor set
        for (set_idx, set) in raw_sets {
            unsafe {
                device.raw.cmd_bind_descriptor_sets(
                    self.cb.raw, 
                    pipeline.pipeline_bind_point, 
                    pipeline.pipeline_layout,
                    *set_idx, 
                    &[*set], 
                    &[]
                );
            }
        }

        Ok(())
    }

    fn bind_descriptor_set(
        &self,
        pipeline: &CommonPipeline,
        set_index: u32,
        bindings: &[DescriptorSetBinding],
    ) {
        let raw_device = &self.context.execution_params.device.raw;

        let pool = {
            let descriptor_pool_ci = vk::DescriptorPoolCreateInfo::builder()
                .max_sets(1)
                .pool_sizes(&pipeline.descriptor_pool_sizes);
    
            unsafe { raw_device.create_descriptor_pool(&descriptor_pool_ci, None) }.unwrap()
        };

        // release in next frame
        self.context.execution_params.device.defer_release(pool);

        // create descriptor set
        let descriptor_set = {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(pool)
                .set_layouts(std::slice::from_ref(
                    &pipeline.descriptor_set_layouts[set_index as usize],
                ));
    
            unsafe { raw_device.allocate_descriptor_sets(&allocate_info) }.unwrap()[0]
        };

        let set_layout_info = if let Some(set_layout_info) = pipeline.set_layout_infos.get(set_index as usize) {
            set_layout_info
        } else {
            panic!("Expect set {} but not found in pipeline shader!", set_index)
        };

        let image_infos = TempList::new();
        let buffer_infos = TempList::new();
        // TODO: use some memory arena to avoid frequently allocations and deallocations
        let mut dynamic_offsets: Vec<u32> = Vec::new();

        // update descriptor set and bind it
        let descriptor_writes = bindings.iter()
            .enumerate()
            // the binding must be defined in the pipeline shader
            .filter(|(binding_idx, _)| set_layout_info.contains_key(&(*binding_idx as u32)))
            .map(|(binding_idx, binding)| {
                let write = vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(binding_idx as u32)
                    .dst_array_element(0);

                match binding {
                    DescriptorSetBinding::Image(image) => write
                        .descriptor_type(match image.image_layout {
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                                vk::DescriptorType::SAMPLED_IMAGE
                            }
                            vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                            _ => unimplemented!(),
                        })
                        .image_info(std::slice::from_ref(image_infos.add(*image)))
                        .build(),
                    DescriptorSetBinding::ImageArray(images) => {
                        assert!(!images.is_empty());

                        write.descriptor_type(match images[0].image_layout {
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                                vk::DescriptorType::SAMPLED_IMAGE
                            }
                            vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                            _ => unimplemented!(),
                        })
                        .image_info(images.as_slice())
                        .build()
                    },
                    DescriptorSetBinding::Buffer(buffer) => write
                        // TODO: all is storage buffer??
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer)))
                        .build(),
                    DescriptorSetBinding::DynamicBuffer { buffer_info, offset } => {
                        dynamic_offsets.push(*offset);
                        write
                            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                            .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                            .build()
                    },
                    DescriptorSetBinding::DynamicStorageBuffer { buffer_info, offset } => {
                        dynamic_offsets.push(*offset);
                        write
                            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER_DYNAMIC)
                            .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                            .build()
                    }
                }
            })
            .collect::<Vec<_>>();

        unsafe {
            raw_device.update_descriptor_sets(&descriptor_writes, &[]);

            raw_device.cmd_bind_descriptor_sets(
                self.cb.raw, 
                pipeline.pipeline_bind_point, 
                pipeline.pipeline_layout, 
                set_index, 
                &[descriptor_set], 
                dynamic_offsets.as_slice()
            );
        }
    }
}