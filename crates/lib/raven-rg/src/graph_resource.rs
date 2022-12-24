use std::{marker::PhantomData, sync::Arc};

use vk_sync::AccessType;

use raven_rhi::backend::{
    Image, ImageDesc,
    Buffer, BufferDesc, 
    RasterPipelineDesc, ComputePipelineDesc, PipelineShaderDesc
};
#[cfg(feature = "gpu_ray_tracing")]
use raven_rhi::backend::{RayTracingAccelerationStructure, RayTracingPipelineDesc};

use crate::resource::{ResourceDesc, Resource, ResourceView};
#[cfg(feature = "gpu_ray_tracing")]
use crate::resource::{RayTracingAccelStructDesc};

/// Description for render graph resource.
/// 
/// Because GraphResource can NOT have any generic type parameters,
/// we have to create a ResourceDesc for render graph to hold the data.
/// But GraphResourceDesc is the same as ResourceDesc in resource.rs .
#[derive(Clone, Copy, Debug)]
pub enum GraphResourceDesc {
    Image(ImageDesc),
    Buffer(BufferDesc),
    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct(RayTracingAccelStructDesc),
}

/// Resource which will be created and hold by render graph.
/// 
/// Render graph will assumed that this resource will be used permanently in this application lifetime.
#[derive(Clone)]
pub(crate) struct GraphResourceCreatedData {
    pub desc: GraphResourceDesc,
}

/// Resource which can be imported from outside the render graph.
/// 
/// Notice that SwapchainImage has no extra data, because we delayed the swapchain import when we actually need to use swapchain image.
#[derive(Clone)]
pub(crate) enum GraphResourceImportedData {
    Image {
        raw: Arc<Image>,
        access: AccessType,
    },
    Buffer {
        raw: Arc<Buffer>,
        access: AccessType,
    },
    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct {
        raw: Arc<RayTracingAccelerationStructure>,
        access: AccessType,
    },
    SwapchainImage,
}

/// Render graph resource.
#[derive(Clone)]
pub(crate) enum GraphResource {
    /// Will be lately created and owned by render graph
    Created(GraphResourceCreatedData),
    /// Imported from outer resource.
    Imported(GraphResourceImportedData),
}

/// Exportable render graph resource.
pub(crate) enum ExportableGraphResource {
    Image(Handle<Image>),
    Buffer(Handle<Buffer>),
    #[cfg(feature = "gpu_ray_tracing")]
    #[allow(dead_code)] // reason = "We will not going to implement get_or_create_temporal() for RayTracingAccelerationStructure"
    RayTracingAccelStruct(Handle<RayTracingAccelerationStructure>),
}

impl ExportableGraphResource {
    pub(crate) fn handle(&self) -> GraphResourceHandle {
        match self {
            ExportableGraphResource::Image(handle) => handle.handle,
            ExportableGraphResource::Buffer(handle) => handle.handle,
            #[cfg(feature = "gpu_ray_tracing")]
            ExportableGraphResource::RayTracingAccelStruct(handle) => handle.handle,
        }
    }
}

pub(crate) enum ExportedResourceHandle {
    Image(ExportedHandle<Image>),
    Buffer(ExportedHandle<Buffer>),
    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct(ExportedHandle<RayTracingAccelerationStructure>),
}

impl GraphResource {
    pub(crate) fn create<Desc: ResourceDesc>(desc: Desc) -> GraphResource {
        GraphResource::Created(GraphResourceCreatedData{ desc: desc.into() })
    }

    pub(crate) fn import_image(resource: Arc<Image>, access: AccessType) -> GraphResource {
        GraphResource::Imported(GraphResourceImportedData::Image{ raw: resource, access })
    }

    pub(crate) fn import_buffer(resource: Arc<Buffer>, access: AccessType) -> GraphResource {
        GraphResource::Imported(GraphResourceImportedData::Buffer{ raw: resource, access })
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub(crate) fn import_ray_tracing_accel_struct(resource: Arc<RayTracingAccelerationStructure>, access: AccessType) -> GraphResource {
        GraphResource::Imported(GraphResourceImportedData::RayTracingAccelStruct { raw: resource, access })
    }
}

/// Render graph resource handle to the inner resources of the render graph.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub(crate) struct GraphResourceHandle {
    /// Slot id of the resources in the render graph.
    pub(crate) id: u32,
    /// Generation id of current resource.
    pub(crate) generation: u32,
}

impl GraphResourceHandle {
    /// This resource had been expired, step to next generation.
    pub(crate) fn expired(self) -> Self {
        Self {
            id: self.id,
            generation: self.generation.wrapping_add(1),
        }
    }
}

#[derive(Debug)]
/// Handle of any render resource in the render graph.
pub struct Handle<ResourceType: Resource> {
    /// Handle of the render graph resources.
    pub(crate) handle: GraphResourceHandle,
    /// Description of this resource.
    pub(crate) desc: <ResourceType as Resource>::Desc,
    /// Rust: Use PhantomData to tell rust Handle holds a ResourceType object.
    pub(crate) _marker: PhantomData<ResourceType>,
}

impl<ResourceType: Resource> Handle<ResourceType> {
    pub fn desc(&self) -> &<ResourceType as Resource>::Desc {
        &self.desc
    }

    pub(crate) fn clone_unchecked(&self) -> Self {
        Self {
            handle: self.handle,
            desc: self.desc.clone(),
            _marker: PhantomData,
        }
    }
}

impl<ResourceType: Resource> PartialEq for Handle<ResourceType> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<ResourceType: Resource> Eq for Handle<ResourceType> {}

/// It is actually the same thing as Handle.
/// But use different types to distinguish resources under different lifetimes (Or we say, different usage).
/// Because the exported resource must be created, so we do not need the ResourceDesc anymore.
#[derive(Debug)]
pub struct ExportedHandle<ResourceType: Resource> {
    /// Handle of the render graph resources.
    pub(crate) handle: GraphResourceHandle,
    /// Rust: Use PhantomData to tell rust Handle holds a ResourceType object.
    pub(crate) _marker: PhantomData<ResourceType>,
}

impl<ResourceType: Resource> Clone for ExportedHandle<ResourceType> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            _marker: PhantomData,
        }
    }
}

impl<ResourceType: Resource> Copy for ExportedHandle<ResourceType> { }

/// Same as Handle, but add ResourceView as a marker to indicate the view type to be used in renderpass building.
pub struct GraphResourceRef<ResType: Resource, ViewType: ResourceView> {
    pub(crate) handle: GraphResourceHandle,
    //pub(crate) desc: <ResType as Resource>::Desc,
    pub(crate) _marker: PhantomData<(ResType, ViewType)>,
}

pub(crate) struct RenderGraphRasterPipeline {
    pub(crate) desc: RasterPipelineDesc,
    pub(crate) stages: Vec<PipelineShaderDesc>,
}

#[derive(Clone, Copy)]
pub struct GraphRasterPipelineHandle {
    pub(crate) idx: usize,
}

pub(crate) struct RenderGraphComputePipeline {
    pub(crate) desc: ComputePipelineDesc,
}

#[derive(Clone, Copy)]
pub struct GraphComputePipelineHandle {
    pub(crate) idx: usize,
}

#[cfg(feature = "gpu_ray_tracing")]
pub(crate) struct RenderGraphRayTracingPipeline {
    pub(crate) desc: RayTracingPipelineDesc,
    pub(crate) stages: Vec<PipelineShaderDesc>,
}

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Clone, Copy)]
pub struct GraphRayTracingPipelineHandle {
    pub(crate) idx: usize,
}