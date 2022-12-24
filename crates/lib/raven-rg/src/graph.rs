use std::marker::PhantomData;
use std::sync::{Arc};

// WARN: should not directly using render api relative data structures.
use ash::vk;

use raven_rhi::backend::{ImageDesc, Image, Buffer, barrier, ImageType, AccessType};
#[cfg(feature = "gpu_ray_tracing")]
use raven_rhi::backend::RayTracingAccelerationStructure;
use raven_rhi::pipeline_cache::{PipelineCache};

use crate::graph_resource::{
    GraphResource, GraphResourceHandle, 
    ExportableGraphResource, 
    ExportedHandle, 
    GraphResourceCreatedData, GraphResourceDesc, GraphResourceImportedData, RenderGraphRasterPipeline, RenderGraphComputePipeline
};
#[cfg(feature = "gpu_ray_tracing")]
use crate::graph_resource::RenderGraphRayTracingPipeline;
use crate::resource::{Resource, ResourceDesc, TypeEqualTo};
#[cfg(feature = "gpu_ray_tracing")]
use crate::resource::RayTracingAccelStructDesc;

use super::pass::{Pass, PassBuilder};
use super::graph_resource::Handle;
use super::compiled_graph::{RenderGraphPipelineHandles, CompiledRenderGraph};

pub trait RenderGraphImportExportResource
where
    Self: Resource + Sized,
{
    fn import(
        self: Arc<Self>, // imported resource must be outside resources. (i.e. Arc<>)
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> Handle<Self>;

    fn export(
        handle: Handle<Self>,
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> ExportedHandle<Self>;
}

impl RenderGraphImportExportResource for Buffer {
    fn import(
        self: Arc<Self>, // imported resource must be outside resources. (i.e. Arc<>)
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> Handle<Self> {
        let handle = GraphResourceHandle {
            id: render_graph.resources.len() as u32,
            generation: 0,
        };
        render_graph.resources.push(GraphResource::import_buffer(self.clone(), access));
        
        let desc = self.desc;
        Handle {
            handle,
            desc,
            _marker: PhantomData,
        }
    }

    fn export(
        handle: Handle<Self>,
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> ExportedHandle<Self> {
        let exported_handle = ExportedHandle {
            handle: handle.handle,
            _marker: PhantomData,
        };
        render_graph.exported_resources.push((ExportableGraphResource::Buffer(handle), access));
        exported_handle
    }
}

impl RenderGraphImportExportResource for Image {
    fn import(
        self: Arc<Self>, // imported resource must be outside resources. (i.e. Arc<>)
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> Handle<Self> {
        let handle = GraphResourceHandle {
            id: render_graph.resources.len() as u32,
            generation: 0,
        };
        render_graph.resources.push(GraphResource::import_image(self.clone(), access));
        
        let desc = self.desc;
        Handle {
            handle,
            desc,
            _marker: PhantomData,
        }
    }

    fn export(
        handle: Handle<Self>,
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> ExportedHandle<Self> {
        let exported_handle = ExportedHandle {
            handle: handle.handle,
            _marker: PhantomData,
        };
        render_graph.exported_resources.push((ExportableGraphResource::Image(handle), access));
        exported_handle
    }
}

#[cfg(feature = "gpu_ray_tracing")]
impl RenderGraphImportExportResource for RayTracingAccelerationStructure {
    fn import(
        self: Arc<Self>, // imported resource must be outside resources. (i.e. Arc<>)
        render_graph: &mut RenderGraph,
        access: AccessType,
    ) -> Handle<Self> {
        let handle = GraphResourceHandle {
            id: render_graph.resources.len() as u32,
            generation: 0,
        };
        render_graph.resources.push(GraphResource::import_ray_tracing_accel_struct(self.clone(), access));
        
        let desc = RayTracingAccelStructDesc;

        Handle {
            handle,
            desc,
            _marker: PhantomData,
        }
    }

    fn export(
        _handle: Handle<Self>,
        _render_graph: &mut RenderGraph,
        _access: AccessType,
    ) -> ExportedHandle<Self> {
        unimplemented!()
    }
}

/// Render graph.
pub struct RenderGraph {
    /// TODO: maybe add parallel ability to this. Vec is linear and not suitable for doing parallel dispatching.
    pub(crate) passes: Vec<Pass>,
    pub(crate) resources: Vec<GraphResource>,
    pub(crate) exported_resources: Vec<(ExportableGraphResource, AccessType)>,

    pub(crate) raster_pipelines: Vec<RenderGraphRasterPipeline>,
    pub(crate) compute_pipelines: Vec<RenderGraphComputePipeline>,
    #[cfg(feature = "gpu_ray_tracing")]
    pub(crate) ray_tracing_pipelines: Vec<RenderGraphRayTracingPipeline>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            resources: Vec::new(),
            exported_resources: Vec::new(),

            raster_pipelines: Vec::new(),
            compute_pipelines: Vec::new(),
            #[cfg(feature = "gpu_ray_tracing")]
            ray_tracing_pipelines: Vec::new(),
        }
    }

    /// Add a new render pass to the render graph.
    pub fn add_pass<'rg>(&'rg mut self, name: &str) -> PassBuilder<'rg> {
        let curr_pass_idx = self.passes.len();

        PassBuilder {
            rg: self,
            pass: Some(Pass::new_empty(curr_pass_idx, name.to_string())),
        }
    }

    /// Actully add the new pass to the render graph.
    pub(crate) fn finish_add_pass(&mut self, pass: Pass) {
        self.passes.push(pass);
    }

    pub fn new_resource<Desc: ResourceDesc>(
        &mut self,
        desc: Desc, // this is ResourceDesc
    ) -> Handle<<Desc as ResourceDesc>::Resource> 
    where
        Desc: TypeEqualTo<Other = <<Desc as ResourceDesc>::Resource as Resource>::Desc>,
    {
        let handle: Handle<<Desc as ResourceDesc>::Resource> = Handle {
            handle: self.new_raw_resource(desc.clone()),
            // here we need Resource::Desc, it is not ResourceDesc, so we need to tell compiler there two types are the same by using TypeEqualTo trait
            desc: TypeEqualTo::same(desc),
            _marker: PhantomData,
        };

        handle
    }
    
    pub(crate) fn new_raw_resource<Desc: ResourceDesc>(
        &mut self,
        desc: Desc,
    ) -> GraphResourceHandle {
        let handle = GraphResourceHandle {
            id: self.resources.len() as u32,
            generation: 0,
        };

        self.resources.push(GraphResource::create(desc));
        handle
    }

    pub fn import<ResourceType: RenderGraphImportExportResource>(
        &mut self,
        resource: Arc<ResourceType>,
        access: AccessType,
    ) -> Handle<ResourceType> {
        // add this resource into render graph and give back a handle to user.
        RenderGraphImportExportResource::import(resource, self, access)
    }
    
    pub fn export<ResourceType: RenderGraphImportExportResource>(
        &mut self,
        handle: Handle<ResourceType>,
        access: AccessType,
    ) -> ExportedHandle<ResourceType> {
        // add this resource render graph ExportableResource and get back an ExportedHandle to user.
        RenderGraphImportExportResource::export(handle, self, access)
    }

    /// # Safety 
    /// 
    /// DO NOT call this function multiple times in a frame.
    /// There is a problem with this implementation, if user call get_swapchain() multiple times,
    /// render graph will have multiple swpachain image handles, and this will lead to some crashes.
    pub fn get_swapchain(&mut self, extent: [u32; 2]) -> Handle<Image> {
        // just create a new resource
        let handle = GraphResourceHandle {
            id: self.resources.len() as u32,
            generation: 0,
        };

        self.resources.push(GraphResource::Imported(GraphResourceImportedData::SwapchainImage));

        Handle {
            handle,
            desc: ImageDesc {
                extent: [extent[0], extent[1], 1],
                image_type: ImageType::Tex2d,
                usage: vk::ImageUsageFlags::default(),
                flags: vk::ImageCreateFlags::empty(),
                format: vk::Format::B8G8R8A8_UNORM,
                sample: vk::SampleCountFlags::TYPE_1,
                tiling: vk::ImageTiling::OPTIMAL,
                array_elements: 1,
                mip_levels: 1,
            },
            _marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResourceLifetime {
    /// The pass idx of the last access pass.
    last_access: Option<usize>,
}

#[derive(Clone, Debug)]
// WARN: should NOT directly using graphic api relative data structures.
pub(crate) enum ResourceUsage {
    Empty,
    Image(vk::ImageUsageFlags),
    Buffer(vk::BufferUsageFlags),

    #[cfg(feature = "gpu_ray_tracing")]
    #[allow(dead_code)]
    RayTracingAccelStruct,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        ResourceUsage::Empty
    }
}

pub(crate) struct AnalyzedResourceInfos {
    #[allow(dead_code)]
    pub(crate) lifetimes: Vec<ResourceLifetime>,
    pub(crate) resource_usages: Vec<ResourceUsage>,
}

/// Compile Render Graph relative functions.
impl RenderGraph {
    fn analyze_resources(&self) -> AnalyzedResourceInfos {
        // lifetime infos initialization
        let mut lifetimes: Vec<ResourceLifetime> = self.resources.iter()
            .map(|res| {
                match res {
                    GraphResource::Created(_) => {
                        ResourceLifetime {
                            last_access: None,
                        }
                    },
                    GraphResource::Imported(_) => {
                        ResourceLifetime {
                            last_access: Some(0),
                        }
                    },
                }
            })
            .collect();

        let mut resource_usages: Vec<ResourceUsage> = vec![Default::default(); self.resources.len()];

        // init all created resources usage flags
        for (idx, resource) in self.resources.iter().enumerate() {
            match resource {
                GraphResource::Created(GraphResourceCreatedData {
                    desc: GraphResourceDesc::Image(desc),
                }) => {
                    resource_usages[idx] = ResourceUsage::Image(desc.usage);
                }
                GraphResource::Created(GraphResourceCreatedData {
                    desc: GraphResourceDesc::Buffer(desc),
                }) => {
                    resource_usages[idx] = ResourceUsage::Buffer(desc.usage);
                },
                #[cfg(feature = "gpu_ray_tracing")]
                GraphResource::Created(GraphResourceCreatedData {
                    desc: GraphResourceDesc::RayTracingAccelStruct(_),
                }) => unimplemented!(),
                GraphResource::Imported(GraphResourceImportedData::SwapchainImage) => {
                    resource_usages[idx] = ResourceUsage::Image(vk::ImageUsageFlags::default());
                },
                _ => {}
            }
        }

        // iterate over all passes' resources
        for (pass_idx, pass) in self.passes.iter().enumerate() {
            for pass_resource_handle in pass.inputs.iter().chain(pass.outputs.iter()) {
                let resource_index = pass_resource_handle.handle.id as usize;

                let lifetime = &mut lifetimes[resource_index];
                // update the last access pass
                lifetime.last_access = Some(
                    lifetime.last_access
                        .map(|last_access| last_access.max(pass_idx))
                        .unwrap_or(pass_idx),
                );

                let access_mask = barrier::get_access_info(pass_resource_handle.access.access_type).access_mask;

                match &self.resources[resource_index] {
                    // image usage flags update
                    GraphResource::Created(GraphResourceCreatedData {
                        desc: GraphResourceDesc::Image(_),
                    }) | 
                    GraphResource::Imported(GraphResourceImportedData::SwapchainImage) => {
                        let image_usage: vk::ImageUsageFlags = image_access_mask_to_usage_flags(access_mask);

                        if let ResourceUsage::Image(image) = &mut resource_usages[resource_index] {
                            *image |= image_usage;
                        }
                    }
                    GraphResource::Imported(GraphResourceImportedData::Image { raw, .. }) => {
                        // insert usage
                        let mut image_usage: vk::ImageUsageFlags = image_access_mask_to_usage_flags(access_mask);
                        image_usage |= raw.desc.usage;

                        resource_usages[resource_index] = ResourceUsage::Image(image_usage);
                    }

                    // buffer usage flags update
                    GraphResource::Created(GraphResourceCreatedData {
                        desc: GraphResourceDesc::Buffer(_),
                    }) => {
                        let buffer_usage: vk::BufferUsageFlags = buffer_access_mask_to_usage_flags(access_mask);

                        if let ResourceUsage::Buffer(buffer) = &mut resource_usages[resource_index] {
                            *buffer |= buffer_usage;
                        } else {
                            panic!("Expect {} to be buffer resource!", resource_index);
                        }
                    }
                    GraphResource::Imported(GraphResourceImportedData::Buffer { raw, .. }) => {
                        // insert usage
                        let mut buffer_usage: vk::BufferUsageFlags = buffer_access_mask_to_usage_flags(access_mask);

                        buffer_usage |= raw.desc.usage;
                        resource_usages[resource_index] = ResourceUsage::Buffer(buffer_usage);
                    }

                    // ray tracing usage flags are ignored for now
                    #[cfg(feature = "gpu_ray_tracing")]
                    GraphResource::Created(GraphResourceCreatedData {
                        desc: GraphResourceDesc::RayTracingAccelStruct(_),
                    }) => {}
                    #[cfg(feature = "gpu_ray_tracing")]
                    GraphResource::Imported(GraphResourceImportedData::RayTracingAccelStruct { .. }) => {}
                };
            }
        }

        // for those exported resources, expand their lifetimes
        for (res, access_type) in &self.exported_resources {
            let resource_index = res.handle().id as usize;

            lifetimes[resource_index].last_access = Some(self.passes.len().saturating_sub(1));

            // update usage flags from access type info
            if *access_type != vk_sync::AccessType::Nothing {
                let access_mask = barrier::get_access_info(*access_type).access_mask;

                match res {
                    ExportableGraphResource::Image(_) => {
                        if let ResourceUsage::Image(image) = &mut resource_usages[resource_index] {
                            *image |= image_access_mask_to_usage_flags(access_mask);
                        } else {
                            panic!("Expect {} to be image resource!", resource_index);
                        }
                    }
                    ExportableGraphResource::Buffer(_) => {
                        if let ResourceUsage::Buffer(buffer) = &mut resource_usages[resource_index] {
                            *buffer |= buffer_access_mask_to_usage_flags(access_mask);
                        } else {
                            panic!("Expect {} to be buffer resource!", resource_index);
                        }
                    }
                    
                    // ray tracing usage flags are ignored for now
                    #[cfg(feature = "gpu_ray_tracing")]
                    ExportableGraphResource::RayTracingAccelStruct(_) => {}
                }
            }
        }

        AnalyzedResourceInfos {
            lifetimes,
            resource_usages
        }
    }

    // Resolve resource information from passes and register its pipelines.
    pub(crate) fn compile(self, pipeline_cache: &mut PipelineCache) -> CompiledRenderGraph {
        let resource_infos = self.analyze_resources();

        let raster_pipeline_handles = self.raster_pipelines.iter()
            .map(|rg_raster| pipeline_cache.register_raster_pipeline(&rg_raster.stages, &rg_raster.desc))
            .collect::<Vec<_>>();

        let compute_pipeline_handles = self.compute_pipelines.iter()
            .map(|rg_compute| pipeline_cache.register_compute_pipeline(&rg_compute.desc))
            .collect::<Vec<_>>();

        #[cfg(feature = "gpu_ray_tracing")]
        let ray_tracing_pipeline_handles = self.ray_tracing_pipelines.iter()
            .map(|rg_rt| pipeline_cache.register_ray_tracing_pipeline(&rg_rt.stages, &rg_rt.desc))
            .collect::<Vec<_>>();

        CompiledRenderGraph {
            render_graph: self,
            resource_infos,

            pipelines: RenderGraphPipelineHandles {
                raster_pipeline_handles,
                compute_pipeline_handles,
                #[cfg(feature = "gpu_ray_tracing")]
                ray_tracing_pipeline_handles,
            },
        }
    }
}

fn image_access_mask_to_usage_flags(access_mask: vk::AccessFlags) -> vk::ImageUsageFlags {
    match access_mask {
        vk::AccessFlags::SHADER_READ => vk::ImageUsageFlags::SAMPLED,
        vk::AccessFlags::SHADER_WRITE => vk::ImageUsageFlags::STORAGE,
        vk::AccessFlags::COLOR_ATTACHMENT_READ => vk::ImageUsageFlags::COLOR_ATTACHMENT,
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE => vk::ImageUsageFlags::COLOR_ATTACHMENT,
        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ => {
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
        }
        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE => {
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
        }
        vk::AccessFlags::TRANSFER_READ => vk::ImageUsageFlags::TRANSFER_SRC,
        vk::AccessFlags::TRANSFER_WRITE => vk::ImageUsageFlags::TRANSFER_DST,

        _ if access_mask == vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE => {
            vk::ImageUsageFlags::STORAGE
        }

        // Appears with DepthAttachmentWriteStencilReadOnly
        _ if access_mask
            == vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE =>
        {
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
        }
        _ => panic!("Invalid image access mask: {:?}", access_mask),
    }
}

fn buffer_access_mask_to_usage_flags(access_mask: vk::AccessFlags) -> vk::BufferUsageFlags {
    match access_mask {
        vk::AccessFlags::INDIRECT_COMMAND_READ => vk::BufferUsageFlags::INDIRECT_BUFFER,
        vk::AccessFlags::INDEX_READ => vk::BufferUsageFlags::INDEX_BUFFER,
        vk::AccessFlags::VERTEX_ATTRIBUTE_READ => vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER,
        vk::AccessFlags::UNIFORM_READ => vk::BufferUsageFlags::UNIFORM_BUFFER,
        vk::AccessFlags::SHADER_READ => vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER,
        vk::AccessFlags::SHADER_WRITE => vk::BufferUsageFlags::STORAGE_BUFFER,
        vk::AccessFlags::TRANSFER_READ => vk::BufferUsageFlags::TRANSFER_SRC,
        vk::AccessFlags::TRANSFER_WRITE => vk::BufferUsageFlags::TRANSFER_DST,
        _ if access_mask == vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE => {
            vk::BufferUsageFlags::STORAGE_BUFFER
        }
        _ => panic!("Invalid buffer access mask: {:?}", access_mask),
    }
}