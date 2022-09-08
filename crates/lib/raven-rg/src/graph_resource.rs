use std::{marker::PhantomData, sync::Arc};

use vk_sync::AccessType;

use raven_rhi::backend::{Image, ImageDesc, Buffer, BufferDesc, RasterPipelineDesc, ComputePipelineDesc, PipelineShaderDesc};

use crate::resource::ResourceDesc;
use super::graph::RenderGraph;
use super::resource::{Resource, ResourceView};

/// Because GraphResource can NOT have any generic type parameters,
/// we have to create a ResourceDesc for render graph to hold the data.
/// But GraphResourceDesc is the same as ResourceDesc in resource.rs .
#[derive(Clone, Copy, Debug)]
pub enum GraphResourceDesc {
    Image(ImageDesc),
    Buffer(BufferDesc),
}

pub(crate) struct GraphResourceCreatedData {
    pub desc: GraphResourceDesc,
}

pub(crate) enum GraphResourceImportedData {
    Image {
        raw: Arc<Image>,
        access: AccessType,
    },
    Buffer{
        raw: Arc<Buffer>,
        access: AccessType,
    },
    SwapchainImage,
}

pub(crate) enum GraphResource {
    Created(GraphResourceCreatedData),
    Imported(GraphResourceImportedData),
}

pub(crate) enum ExportableGraphResource {
    Image(Handle<Image>),
    Buffer(Handle<Buffer>),
}

impl ExportableGraphResource {
    pub(crate) fn handle(&self) -> GraphResourceHandle {
        match self {
            ExportableGraphResource::Image(handle) => handle.handle,
            ExportableGraphResource::Buffer(handle) => handle.handle,
        }
    }
}

pub(crate) enum ExportedResourceHandle {
    Image(ExportedHandle<Image>),
    Buffer(ExportedHandle<Buffer>),
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
pub struct ExportedHandle<ResourceType: Resource> {
    /// Handle of the render graph resources.
    pub(crate) handle: GraphResourceHandle,
    /// Rust: Use PhantomData to tell rust Handle holds a ResourceType object.
    pub(crate) _marker: PhantomData<ResourceType>,
}

/// Same as Handle, but add ResourceView as a marker to indicate the view type to be used in renderpass building.
pub struct GraphResourceRef<ResType: Resource, ViewType: ResourceView> {
    pub(crate) handle: GraphResourceHandle,
    pub(crate) desc: <ResType as Resource>::Desc,
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