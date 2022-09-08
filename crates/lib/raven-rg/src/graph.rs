use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::collections::hash_map;

use raven_rhi::backend::{Device, ImageDesc, Image, Buffer, BufferDesc, barrier};
use vk_sync::AccessType;
use anyhow::Context;
// WARN: should not directly using render api relative data structures.
use ash::vk;

use crate::graph_resource::{
    GraphResource, GraphResourceHandle, 
    ExportableGraphResource, 
    ExportedHandle, ExportedResourceHandle, 
    GraphResourceCreatedData, GraphResourceDesc, GraphResourceImportedData, RenderGraphRasterPipeline, RenderGraphComputePipeline
};
use crate::resource::{Resource, ResourceDesc, TypeEqualTo};

use super::pass::{Pass, PassBuilder};
use super::graph_resource::Handle;

pub enum RenderGraphState {
    Pending,
    Compiling,
    Executing,
    Dying,
}

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

/// Render graph.
pub struct RenderGraph {
    /// TODO: maybe add parallel ability to this. Vec is linear and not suitable for doing parallel dispatching.
    passes: Vec<Pass>,
    resources: Vec<GraphResource>,
    exported_resources: Vec<(ExportableGraphResource, AccessType)>,
    pub(crate) graph_state: RenderGraphState,

    pub(crate) raster_pipelines: Vec<RenderGraphRasterPipeline>,
    pub(crate) compute_pipelines: Vec<RenderGraphComputePipeline>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            resources: Vec::new(),
            exported_resources: Vec::new(),
            graph_state: RenderGraphState::Pending,

            raster_pipelines: Vec::new(),
            compute_pipelines: Vec::new(),
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
}

#[derive(Debug)]
struct ResourceLifetime {
    /// The pass idx of the last access pass.
    last_access: Option<usize>,
}

#[derive(Clone, Debug)]
// WARN: should NOT directly using render api relative data structures.
enum ResourceUsage {
    Empty,
    Image(vk::ImageUsageFlags),
    Buffer(vk::BufferUsageFlags),
}

impl Default for ResourceUsage {
    fn default() -> Self {
        ResourceUsage::Empty
    }
}

struct AnalyzedResourceInfo {
    lifetimes: Vec<ResourceLifetime>,
    resource_usages: Vec<ResourceUsage>,
}

/// Compile Render Graph relative functions.
impl RenderGraph {
    fn analyze_resources(&self) -> AnalyzedResourceInfo {
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
                }
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
                    })
                    | GraphResource::Imported(GraphResourceImportedData::Image { .. })
                    | GraphResource::Imported(GraphResourceImportedData::SwapchainImage) => {
                        let image_usage: vk::ImageUsageFlags = image_access_mask_to_usage_flags(access_mask);

                        if let ResourceUsage::Image(image) = &mut resource_usages[resource_index] {
                            *image |= image_usage;
                        } else {
                            panic!("Expect {} to be image resource!", resource_index);
                        }
                    }

                    // buffer usage flags update
                    GraphResource::Created(GraphResourceCreatedData {
                        desc: GraphResourceDesc::Buffer(_),
                    })
                    | GraphResource::Imported(GraphResourceImportedData::Buffer { .. }) => {
                        let buffer_usage: vk::BufferUsageFlags = buffer_access_mask_to_usage_flags(access_mask);

                        if let ResourceUsage::Buffer(buffer) = &mut resource_usages[resource_index] {
                            *buffer |= buffer_usage;
                        } else {
                            panic!("Expect {} to be buffer resource!", resource_index);
                        }
                    }
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
                }
            }
        }

        AnalyzedResourceInfo {
            lifetimes,
            resource_usages
        }
    }

    // Resolve resource informations from passes and exported resources.
    pub fn compile(self) -> CompiledRenderGraph {
        assert!(matches!(self.graph_state, RenderGraphState::Compiling));

        let resource_infos = self.analyze_resources();

        

        CompiledRenderGraph {
            resource_infos,
        }
    }
}

pub struct CompiledRenderGraph {
    resource_infos: AnalyzedResourceInfo,
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

/// Cainozoic Render Graph.
/// It is used to prepare resources and contexts for all the passes.
/// User can create temporary resources for preparing.
pub struct CainozoicRenderGraph {
    device: Arc<Device>,
    render_graph: RenderGraph,
    temporal_resources: HashMap<TemporalResourceKey, TemporalResourceState>,
}

/// CainozoicRenderGraph IS A render graph, but in specific lifetime.
impl std::ops::Deref for CainozoicRenderGraph {
    type Target = RenderGraph;

    fn deref(&self) -> &Self::Target {
        &self.render_graph
    }
}

/// CainozoicRenderGraph IS A render graph, but in specific lifetime.
impl std::ops::DerefMut for CainozoicRenderGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.render_graph
    }
}

impl CainozoicRenderGraph {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            render_graph: RenderGraph::new(),
            temporal_resources: HashMap::new(),
        }
    }

    pub fn device(&self) -> &Device {
        self.device.as_ref()
    }
}

/// Resource that is temporary used by render graph.
/// Because it is temporal, it is not a handle of render graph.
#[derive(Clone)]
pub(crate) enum TemporalResource {
    Image(Arc<Image>),
    Buffer(Arc<Buffer>),
}

/// TemporalResourceState itself contains the resource itself.
pub(crate) enum TemporalResourceState {
    /// Resource that is created but not yet referenced by render graph.
    Inert {
        resource: TemporalResource,
        access: AccessType,
    },
    /// Resource that is imported as read resource by some pass in the render graph.
    Imported {
        resource: TemporalResource,
        handle: ExportableGraphResource,
    },
    /// Resource that is imported as write resource by some pass in the render graph.
    Exported {
        resource: TemporalResource,
        handle: ExportedResourceHandle,
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub struct TemporalResourceKey(String);

impl<'a> From<&'a str> for TemporalResourceKey {
    fn from(s: &'a str) -> Self {
        TemporalResourceKey(String::from(s))
    }
}

impl From<String> for TemporalResourceKey {
    fn from(s: String) -> Self {
        TemporalResourceKey(s)
    }
}

pub trait GetOrCreateTemporal<Desc: ResourceDesc> {
    /// Get Or Create a temporal resource for user to use.
    /// The newly created or fetched resources will be imported into the render graph is user Get Or Create a temporal resource.
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: Desc,
    ) -> anyhow::Result<Handle<<Desc as ResourceDesc>::Resource>>
    where
        Desc: TypeEqualTo<Other = <<Desc as ResourceDesc>::Resource as Resource>::Desc>;
}

impl GetOrCreateTemporal<ImageDesc> for CainozoicRenderGraph {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: ImageDesc,
    ) -> anyhow::Result<Handle<<ImageDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.entry(key.clone()) {
            hash_map::Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                
                match state {
                    TemporalResourceState::Inert { resource, access } => {
                        // this clone is actually a Arc::clone().
                        let resource = resource.clone();
                    
                        match &resource {
                            TemporalResource::Image(image) => {
                                let handle = self.render_graph.import(image.clone(), *access);
                                // DO NOT forget to changed the state of this resource
                                *state = TemporalResourceState::Imported { 
                                    resource, 
                                    handle: ExportableGraphResource::Image(handle.clone_unchecked())
                                };
                                Ok(handle)
                            }
                            TemporalResource::Buffer(..) => {
                                anyhow::bail!("Required an image resource, but pass in a buffer name! {:?}", key)
                            },
                        }
                    },
                    TemporalResourceState::Imported { .. } => {
                        Err(anyhow::anyhow!("This temporal resource is already taken by {:?}", key))
                    },
                    TemporalResourceState::Exported { .. } => {
                        unreachable!()
                    }
                }
            },
            hash_map::Entry::Vacant(entry) => {
                let resource = Arc::new(
                    self.device
                        .create_image(desc)
                        .with_context(|| format!("Failed to create image: {:?}", desc))?,
                );
                let handle = self.render_graph.import(resource.clone(), AccessType::Nothing);
                entry.insert(TemporalResourceState::Imported {
                    resource: TemporalResource::Image(resource),
                    handle: ExportableGraphResource::Image(handle.clone_unchecked()),
                });
                Ok(handle)
            }
        }
    }
}

impl GetOrCreateTemporal<BufferDesc> for CainozoicRenderGraph {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: BufferDesc,
    ) -> anyhow::Result<Handle<<BufferDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.entry(key.clone()) {
            hash_map::Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                
                match state {
                    TemporalResourceState::Inert { resource, access } => {
                        // this clone is actually a Arc::clone().
                        let resource = resource.clone();
                    
                        match &resource {
                            TemporalResource::Buffer(buffer) => {
                                let handle = self.render_graph.import(buffer.clone(), *access);
                                // DO NOT forget to changed the state of this resource
                                *state = TemporalResourceState::Imported { 
                                    resource, 
                                    handle: ExportableGraphResource::Buffer(handle.clone_unchecked())
                                };
                                Ok(handle)
                            }
                            TemporalResource::Image(..) => {
                                anyhow::bail!("Required a buffer resource, but pass in a image name! {:?}", key)
                            },
                        }
                    },
                    TemporalResourceState::Imported { .. } => {
                        Err(anyhow::anyhow!("This temporal resource is already taken by {:?}", key))
                    },
                    TemporalResourceState::Exported { .. } => {
                        unreachable!()
                    }
                }
            },
            hash_map::Entry::Vacant(entry) => {
                let resource = Arc::new(
                    self.device
                        .create_buffer(desc, "render graph temporal resource")
                        .with_context(|| format!("Failed to create buffer: {:?}", desc))?,
                );
                let handle = self.render_graph.import(resource.clone(), AccessType::Nothing);
                entry.insert(TemporalResourceState::Imported {
                    resource: TemporalResource::Buffer(resource),
                    handle: ExportableGraphResource::Buffer(handle.clone_unchecked()),
                });
                Ok(handle)
            }
        }
    }
}

/// Just a wrapper over TemporalResourceState.
/// Use a new type to distinguish between exported resources.
pub struct ExportedTemporalResourceState(pub(crate) HashMap<TemporalResourceKey, TemporalResourceState>);

impl CainozoicRenderGraph {
    pub fn export_all_imported_resources(self) -> (RenderGraph, ExportedTemporalResourceState) {
        let mut render_graph = self.render_graph;
        let mut temporal_resources = self.temporal_resources;

        for state in temporal_resources.values_mut() {
            match state {
                TemporalResourceState::Inert { .. } => {
                    // if the resources are not referenced by render graph, it is no need to export them.
                }
                TemporalResourceState::Imported { resource, handle } => match handle {
                    ExportableGraphResource::Image(handle) => {
                        let handle = render_graph.export(handle.clone_unchecked(), AccessType::Nothing);

                        // change state from imported to exported
                        *state = TemporalResourceState::Exported {
                            resource: resource.clone(),
                            handle: ExportedResourceHandle::Image(handle),
                        }
                    }
                    ExportableGraphResource::Buffer(handle) => {
                        let handle = render_graph.export(handle.clone_unchecked(), AccessType::Nothing);

                        // change state from imported to exported
                        *state = TemporalResourceState::Exported {
                            resource: resource.clone(),
                            handle: ExportedResourceHandle::Buffer(handle),
                        }
                    }
                },
                TemporalResourceState::Exported { .. } => {
                    // there can be any exported resources!
                    unreachable!()
                }
            }
        }

        render_graph.graph_state = RenderGraphState::Compiling;
        (render_graph, ExportedTemporalResourceState(temporal_resources))
    }
}