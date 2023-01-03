use std::{sync::Arc, collections::HashMap};

// TODO: remove this (render graph should not directly contain any graphic API)
use ash::vk;
use arrayvec::ArrayVec;

use raven_rhi::{
    backend::{Device, ImageViewDesc, Image, Buffer, RasterPipeline, ComputePipeline, CommandBuffer, RenderPass},
    backend::constants,
    backend::RhiError,
    backend::{renderpass::FrameBufferCacheKey, descriptor},
    backend::{pipeline::CommonPipeline},
    backend::{descriptor::DescriptorSetBinding},
    dynamic_buffer::DynamicBuffer,
};
#[cfg(feature = "gpu_ray_tracing")]
use raven_rhi::backend::{RayTracingPipeline, RayTracingAccelerationStructure};

use crate::{graph_executor::{ExecutionParams}, resource::ResourceView};

use super::{
    resource::{Srv, Uav, Rt},
    compiled_graph::{RenderGraphPipelineHandles, RegisteredResource, GraphPreparedResourceRef},
    graph_resource::{GraphResourceHandle, GraphResourceRef, GraphRasterPipelineHandle, GraphComputePipelineHandle},
};
#[cfg(feature = "gpu_ray_tracing")]
use super::graph_resource::{GraphRayTracingPipelineHandle};

pub struct PassImageBinding {
    handle: GraphResourceHandle,
    view_desc: ImageViewDesc,
    layout: vk::ImageLayout,
}

pub struct PassBufferBinding {
    handle: GraphResourceHandle,
}

#[cfg(feature = "gpu_ray_tracing")]
pub struct PassRayTracingAccelStructBinding {
    handle: GraphResourceHandle,
}

pub enum RenderGraphPassBinding {
    Image(PassImageBinding),
    ImageArray(Vec<PassImageBinding>),
    Buffer(PassBufferBinding),

    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct(PassRayTracingAccelStructBinding),

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

    pub fn with_base_mipmap_level(&mut self, base_level: u16) {
        match self {
            RenderGraphPassBinding::Image(image) => {
                image.view_desc.base_mip_level = base_level as u32;
            },
            RenderGraphPassBinding::ImageArray(images) => {
                for image in images {
                    image.view_desc.base_mip_level = base_level as u32;
                }
            },
            _ => panic!("Try to add ImageAspectFlags to buffers!"),
        }
    }

    pub fn with_mipmap_level_count(&mut self, level_count: u32) {
        match self {
            RenderGraphPassBinding::Image(image) => {
                image.view_desc.level_count = Some(level_count);
            },
            RenderGraphPassBinding::ImageArray(images) => {
                for image in images {
                    image.view_desc.level_count = Some(level_count);
                }
            },
            _ => panic!("Try to add ImageAspectFlags to buffers!"),
        }
    }

    pub fn with_image_view(&mut self, view: vk::ImageViewType) {
        match self {
            RenderGraphPassBinding::Image(image) => {
                image.view_desc.view_type = Some(view);
            },
            RenderGraphPassBinding::ImageArray(images) => {
                for image in images {
                    image.view_desc.view_type = Some(view);
                }
            },
            _ => panic!("Try to add ImageAspectFlags to buffers!"),
        }
    }
}

pub trait RenderGraphPassBindable {
    fn bind(&self) -> RenderGraphPassBinding;
}

impl RenderGraphPassBindable for GraphResourceRef<Buffer, Srv> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Buffer(PassBufferBinding {
            handle: self.handle.clone(),
        })
    }
}

impl RenderGraphPassBindable for GraphResourceRef<Image, Srv> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Image(PassImageBinding {
            handle: self.handle.clone(),
            view_desc: ImageViewDesc::default(),
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        })
    }
}

impl RenderGraphPassBindable for Vec<GraphResourceRef<Image, Srv>> {
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

impl RenderGraphPassBindable for GraphResourceRef<Buffer, Uav> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Buffer(PassBufferBinding {
            handle: self.handle.clone(),
        })
    }
}

impl RenderGraphPassBindable for GraphResourceRef<Image, Uav> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::Image(PassImageBinding {
            handle: self.handle.clone(),
            view_desc: ImageViewDesc::default(),
            layout: vk::ImageLayout::GENERAL,
        })
    }
}

impl RenderGraphPassBindable for Vec<GraphResourceRef<Image, Uav>> {
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

#[cfg(feature = "gpu_ray_tracing")]
impl RenderGraphPassBindable for GraphResourceRef<RayTracingAccelerationStructure, Srv> {
    fn bind(&self) -> RenderGraphPassBinding {
        RenderGraphPassBinding::RayTracingAccelStruct(PassRayTracingAccelStructBinding {
            handle: self.handle.clone(),
        })
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

#[cfg(feature = "gpu_ray_tracing")]
impl IntoPipelineDescriptorBindings for GraphRayTracingPipelineHandle {
    fn into_bindings<'a>(self) -> PipelineBindings<'a, Self> {
        PipelineBindings::new(self)
    }
}

pub struct BoundComputePipeline<'context, 'exec, 'a> {
    context: &'context PassContext<'exec, 'a>,
    pipeline: Arc<ComputePipeline>,
}

impl<'context, 'exec, 'a> BoundComputePipeline<'context, 'exec, 'a> {
    pub fn rebind(&self, set_idx: u32, bindings: &[RenderGraphPassBinding]) -> anyhow::Result<(), RhiError> {
        let bindings = rg_pass_binding_to_descriptor_set_bindings(&self.context.registry, bindings)?;

        descriptor::bind_descriptor_set_in_place(
            self.context.device(), self.context.cb, 
            set_idx, &self.pipeline, bindings.as_slice()
        );

        Ok(())
    }

    pub fn dispatch(
        &self,
        threads: [u32; 3],
    ) {
        let device = self.context.registry.execution_params.device;
        let dispatch_groups = self.pipeline.dispatch_groups;

        unsafe {
            device.raw.cmd_dispatch(
                self.context.cb.raw,
                // divide ceil
                (threads[0] + dispatch_groups[0] - 1) / dispatch_groups[0],
                (threads[1] + dispatch_groups[1] - 1) / dispatch_groups[1],
                (threads[2] + dispatch_groups[2] - 1) / dispatch_groups[2]
            );
        }
    }

    pub fn push_constants(
        &self,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        bytes: &[u8],
    ) {
        let device = self.context.registry.execution_params.device;

        unsafe {
            device.raw.cmd_push_constants(
                self.context.cb.raw, 
                self.pipeline.pipeline.pipeline_layout(),
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
    pub fn rebind(&self, set_idx: u32, bindings: &[RenderGraphPassBinding]) -> anyhow::Result<(), RhiError> {
        let bindings = rg_pass_binding_to_descriptor_set_bindings(&self.context.registry, bindings)?;

        descriptor::bind_descriptor_set_in_place(
            self.context.device(), self.context.cb, 
            set_idx, &self.pipeline, bindings.as_slice()
        );

        Ok(())
    }

    pub fn push_constants(
        &self,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        bytes: &[u8],
    ) {
        let device = self.context.registry.execution_params.device;

        unsafe {
            device.raw.cmd_push_constants(
                self.context.cb.raw, 
                self.pipeline.pipeline_layout(), 
                stage_flags, 
                offset, 
                bytes
            );
        }
    }
}

#[cfg(feature = "gpu_ray_tracing")]
pub struct BoundRayTracingPipeline<'context, 'exec, 'a> {
    context: &'context PassContext<'exec, 'a>,
    pipeline: Arc<RayTracingPipeline>,
}

#[cfg(feature = "gpu_ray_tracing")]
impl<'context, 'exec, 'a> BoundRayTracingPipeline<'context, 'exec, 'a> {
    pub fn push_constants(
        &self,
        stage_flags: vk::ShaderStageFlags,
        offset: u32,
        bytes: &[u8],
    ) {
        let device = self.context.registry.execution_params.device;

        unsafe {
            device.raw.cmd_push_constants(
                self.context.cb.raw, 
                self.pipeline.pipeline_layout(), 
                stage_flags, 
                offset, 
                bytes
            );
        }
    }

    pub fn trace_rays(
        &self,
        threads: [u32; 3]
    ) {
        let device = self.context.registry.execution_params.device;
        let sbt = &self.pipeline.sbt;

        unsafe {
            device.ray_tracing_extensions.ray_tracing_pipeline_khr.cmd_trace_rays(
                self.context.cb.raw,
                &sbt.raygen_shader_binding_table,
                &sbt.miss_shader_binding_table,
                &sbt.hit_shader_binding_table,
                &sbt.callable_shader_binding_table,
                threads[0], threads[1], threads[2]
            );
        }
    }

    pub fn trace_rays_indirect(
        &self,
        args_buffer: GraphResourceRef<Buffer, Srv>,
        buffer_offset: u64,
    ) {
        let device = self.context.registry.execution_params.device;
        let sbt = &self.pipeline.sbt;
        let indirect_buf_base_addr = self.context.registry
            .get_buffer(args_buffer)
            .device_address(self.context.device())
            + buffer_offset;

        unsafe {
            device.ray_tracing_extensions.ray_tracing_pipeline_khr
                .cmd_trace_rays_indirect(
                    self.context.cb.raw,
                    std::slice::from_ref(&sbt.raygen_shader_binding_table),
                    std::slice::from_ref(&sbt.miss_shader_binding_table),
                    std::slice::from_ref(&sbt.hit_shader_binding_table),
                    std::slice::from_ref(&sbt.callable_shader_binding_table),
                    indirect_buf_base_addr
                );
        }
    }
}

pub struct GraphResourceRegistry<'exec, 'a> {
    pub execution_params: &'a ExecutionParams<'exec>,

    pub(crate) pipelines: &'a RenderGraphPipelineHandles,
    pub(crate) registered_resources: &'a Vec<RegisteredResource>,
    pub global_dynamic_buffer: &'a mut DynamicBuffer,
}

impl<'exec, 'a> GraphResourceRegistry<'exec, 'a> {
    // Notice that get_buffer() and get_image() ... all consume a GraphResourceRef<>,
    // not by reference but by ownership.
    // We assume that when user borrows the inner resource handle, it can never bind it into pipeline again.
    // Because pipeline may read this resource reference that user may modified before.
    // 
    // Conclude that when user gets the full controls of the resource,
    // we should also let user take the full responsibility of the pipeline resource binding.

    pub fn get_buffer<View: ResourceView>(&self, buf_ref: GraphResourceRef<Buffer, View>) -> &Buffer {
        self.get_buffer_from_raw_handle::<View>(buf_ref.handle)
    }

    pub fn get_image<View: ResourceView>(&self, img_ref: GraphResourceRef<Image, View>) -> &Image {
        self.get_image_from_raw_handle::<View>(img_ref.handle)
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub fn get_acceleration_structure<View: ResourceView>(&self, accel_ref: GraphResourceRef<RayTracingAccelerationStructure, View>) -> &RayTracingAccelerationStructure {
        self.get_acceleration_structure_from_raw_handle::<View>(accel_ref.handle)
    }

    pub(crate) fn get_image_view_from_raw_handle(&self, handle: GraphResourceHandle, view_desc: &ImageViewDesc) -> anyhow::Result<vk::ImageView, RhiError> {
        let image = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Image(image) => image,
            _ => panic!("Expect image, but pass in an incorrect graph resource handle!"),
        };

        let device = self.execution_params.device;
        image.view(device, &view_desc)
    }

    pub(crate) fn get_image_from_raw_handle<View: ResourceView>(&self, handle: GraphResourceHandle) -> &Image {
        let image = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Image(image) => image,
            _ => panic!("Expect image, but pass in an incorrect graph resource handle!"),
        };

        image
    }

    pub(crate) fn get_buffer_from_raw_handle<View: ResourceView>(&self, handle: GraphResourceHandle) -> &Buffer {
        let buffer = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::Buffer(buffer) => buffer,
            _ => panic!("Expect buffer, but pass in an incorrect graph resource handle!"),
        };

        buffer
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub(crate) fn get_acceleration_structure_from_raw_handle<View: ResourceView>(&self, handle: GraphResourceHandle) -> &RayTracingAccelerationStructure {
        let accel_struct = match self.registered_resources[handle.id as usize].resource.borrow() {
            GraphPreparedResourceRef::RayTracingAccelStruct(accel_struct) => accel_struct,
            _ => panic!("Expect ray tracing acceleration structure, but pass in an incorrect graph resource handle!"),
        };

        accel_struct
    }

    pub(crate) fn get_raster_pipeline(&self, handle: GraphRasterPipelineHandle) -> Arc<RasterPipeline> {
        let pipeline = self.pipelines.raster_pipeline_handles[handle.idx];
        self.execution_params.pipeline_cache.get_raster_pipeline(pipeline)
    }

    pub(crate) fn get_compute_pipeline(&self, handle: GraphComputePipelineHandle) -> Arc<ComputePipeline> {
        let pipeline = self.pipelines.compute_pipeline_handles[handle.idx];
        self.execution_params.pipeline_cache.get_compute_pipeline(pipeline)
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub(crate) fn get_ray_tracing_pipeline(&self, handle: GraphRayTracingPipelineHandle) -> Arc<RayTracingPipeline> {
        let pipeline = self.pipelines.ray_tracing_pipeline_handles[handle.idx];
        self.execution_params.pipeline_cache.get_ray_tracing_pipeline(pipeline)
    }
}

/// Render pass context to give user to do custom command buffer recording ability and etc.
pub struct PassContext<'exec, 'a> {
    /// Command Buffer to record rendering commands to.
    pub cb: &'a CommandBuffer,
    /// Context Relative Resources to be used inside this render pass. 
    pub registry: GraphResourceRegistry<'exec, 'a>,
}

impl<'exec, 'a> PassContext<'exec, 'a> {
    
    #[inline]
    pub fn device(&self) -> &Device {
        &self.registry.execution_params.device
    }

    #[inline]
    pub fn global_dynamic_buffer(&mut self) -> &mut DynamicBuffer {
        self.registry.global_dynamic_buffer
    } 

    pub fn begin_render_pass(
        &mut self,
        render_pass: &RenderPass,
        extent: [u32; 2],
        color_attachments: &[(GraphResourceRef<Image, Rt>, &ImageViewDesc)],
        depth_attachment: Option<(GraphResourceRef<Image, Rt>, &ImageViewDesc)>,
    ) -> anyhow::Result<(), RhiError> {
        let device = self.registry.execution_params.device;

        // get or create the framebuffer from the cache
        let framebuffer = render_pass.frame_buffer_cache.get_or_create(device, FrameBufferCacheKey::new(
            extent, 
            color_attachments.iter().map(|(refer, _)| {
                // TODO: is this verbose?
                //&refer.desc
                &self.registry.get_image_from_raw_handle::<Rt>(refer.handle).desc
            }), 
            depth_attachment.as_ref().map(|(refer, _)| {
                //&refer.desc
                &self.registry.get_image_from_raw_handle::<Rt>(refer.handle).desc
            })
        ));

        // collect all image views
        let attachments = color_attachments.iter()
            .chain(depth_attachment.as_ref().into_iter())
            .map(|(refer, view)| self.registry.get_image_view_from_raw_handle(refer.handle, &view))
            .collect::<anyhow::Result<ArrayVec<vk::ImageView, { constants::MAX_RENDERPASS_ATTACHMENTS + 1 }>, RhiError>>();
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

    pub fn set_depth_bias(&self, bias_constant: f32, clamp: f32, slope_factor: f32) {
        unsafe {
            self.device().raw.cmd_set_depth_bias(
                self.cb.raw,
                bias_constant,
                clamp,
                slope_factor
            );
        }
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

    pub fn bind_raster_pipeline(&self, bindings: PipelineBindings<'_, GraphRasterPipelineHandle>) -> anyhow::Result<BoundRasterPipeline, RhiError> {
        let pipeline = self.registry.get_raster_pipeline(bindings.pipeline_handle);
        self.bind_pipeline(self.registry.execution_params.device, pipeline.as_ref(), &bindings.set_layouts, &bindings.raw_sets)?;

        Ok(BoundRasterPipeline {
            context: self,
            pipeline,
        })
    }

    pub fn bind_compute_pipeline(&self, bindings: PipelineBindings<'_, GraphComputePipelineHandle>) -> anyhow::Result<BoundComputePipeline, RhiError> {
        let pipeline = self.registry.get_compute_pipeline(bindings.pipeline_handle);
        self.bind_pipeline(self.registry.execution_params.device, pipeline.as_ref(), &bindings.set_layouts, &bindings.raw_sets)?;

        Ok(BoundComputePipeline {
            context: self,
            pipeline,
        })
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub fn bind_ray_tracing_pipeline(&self, bindings: PipelineBindings<'_, GraphRayTracingPipelineHandle>) -> anyhow::Result<BoundRayTracingPipeline, RhiError> {
        let pipeline = self.registry.get_ray_tracing_pipeline(bindings.pipeline_handle);
        self.bind_pipeline(self.registry.execution_params.device, pipeline.as_ref(), &bindings.set_layouts, &bindings.raw_sets)?;

        Ok(BoundRayTracingPipeline {
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
    ) -> anyhow::Result<(), RhiError> {
        let pipeline_info = &pipeline.pipeline_info;

        // bind pipeline
        unsafe {
            device.raw
                .cmd_bind_pipeline(self.cb.raw, pipeline.pipeline_bind_point(), pipeline.pipeline());
        }
        
        // bind engine global frame constants
        // TODO: do we really need to bind it every time bound pipeline?
        if pipeline_info.set_layout_infos.get(2).is_some() {
            unsafe {
                device.raw.cmd_bind_descriptor_sets(
                    self.cb.raw, 
                    pipeline.pipeline_bind_point(), 
                    pipeline.pipeline_layout(),
                    2,
                    &[self.registry.execution_params.global_constants_set], 
                    &[
                        // binding 0
                        self.registry.execution_params.draw_frame_context_layout.frame_constants_offset
                    ]
                );
            }
        }

        // create and bind pipeline's descriptor sets
        for (set_idx, bindings) in set_layout.bindings.iter() {
            // trying to bind a resource that is not defined in pipeline's shader
            if pipeline_info.set_layout_infos.get(*set_idx as usize).is_none() {
                continue;
            }

            let bindings = rg_pass_binding_to_descriptor_set_bindings(&self.registry, bindings)?;

            descriptor::bind_descriptor_set_in_place(
                device, self.cb, 
                *set_idx, pipeline, bindings.as_slice()
            );
        }

        let device = self.registry.execution_params.device;
        // TODO: unsafe. user specific descriptor set index may collide with raw descriptor set
        for (set_idx, set) in raw_sets {
            unsafe {
                device.raw.cmd_bind_descriptor_sets(
                    self.cb.raw, 
                    pipeline.pipeline_bind_point(), 
                    pipeline.pipeline_layout(),
                    *set_idx, 
                    &[*set], 
                    &[]
                );
            }
        }

        Ok(())
    }
}

fn rg_pass_binding_to_descriptor_set_bindings(
    ctx: &GraphResourceRegistry, 
    bindings: &[RenderGraphPassBinding]
) -> anyhow::Result<Vec<DescriptorSetBinding>, RhiError> {
    let bindings = bindings.iter()
        .map(|pass_binding| {
            Ok(match &pass_binding {
                RenderGraphPassBinding::Image(image) => DescriptorSetBinding::Image(vk::DescriptorImageInfo::builder()
                    .image_layout(image.layout)
                    .image_view(ctx.get_image_view_from_raw_handle(image.handle, &image.view_desc)?)
                    .build()
                ),
                RenderGraphPassBinding::ImageArray(images) => DescriptorSetBinding::ImageArray(
                        images.iter()
                        .map(|image| {
                            Ok(vk::DescriptorImageInfo::builder()
                                .image_layout(image.layout)
                                .image_view(ctx.get_image_view_from_raw_handle(image.handle, &image.view_desc)?)
                                .build())
                        })
                        .collect::<anyhow::Result<Vec<_>, RhiError>>()?
                ),
                RenderGraphPassBinding::Buffer(buffer) => DescriptorSetBinding::Buffer(vk::DescriptorBufferInfo::builder()
                    .buffer(ctx.get_buffer_from_raw_handle::<Srv>(buffer.handle).raw)
                    .range(vk::WHOLE_SIZE)
                    .build()
                ),
                RenderGraphPassBinding::DynamicBuffer(offset) => DescriptorSetBinding::DynamicBuffer {
                    buffer_info: vk::DescriptorBufferInfo::builder()
                        .buffer(ctx.global_dynamic_buffer.buffer.raw)
                        .range(ctx.global_dynamic_buffer.max_uniform_buffer_range() as _)
                        .build(),
                    offset: *offset,
                },
                RenderGraphPassBinding::DynamicStorageBuffer(offset) => DescriptorSetBinding::DynamicStorageBuffer {
                    buffer_info: vk::DescriptorBufferInfo::builder()
                        .buffer(ctx.global_dynamic_buffer.buffer.raw)
                        //.range(self.context.global_dynamic_buffer.max_storage_buffer_range() as _)
                        .range(vk::WHOLE_SIZE)
                        .build(),
                    offset: *offset,
                },
                #[cfg(feature = "gpu_ray_tracing")]
                RenderGraphPassBinding::RayTracingAccelStruct(accel_struct) => DescriptorSetBinding::RayTracingAccelStruct(
                    ctx.get_acceleration_structure_from_raw_handle::<Srv>(accel_struct.handle).raw
                ),
            })
        })
        .collect::<anyhow::Result<Vec<_>, RhiError>>();
    bindings
}