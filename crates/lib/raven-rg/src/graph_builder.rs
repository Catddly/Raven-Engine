use std::sync::Arc;
use std::collections::{hash_map, HashMap};
use anyhow::Context;

use raven_rhi::{
    backend::{Device, Image, ImageDesc, Buffer, BufferDesc, AccessType},
};

use crate::{
    resource::{Resource, ResourceDesc, TypeEqualTo},
    graph_resource::{ExportedResourceHandle, ExportableGraphResource, Handle},
    graph::RenderGraph,
    retired_graph::RetiredRenderGraph,
};

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub struct TemporaryResourceKey(String);

impl<'a> From<&'a str> for TemporaryResourceKey {
    fn from(s: &'a str) -> Self {
        TemporaryResourceKey(String::from(s))
    }
}

impl From<String> for TemporaryResourceKey {
    fn from(s: String) -> Self {
        TemporaryResourceKey(s)
    }
}

#[derive(Default)]
pub struct TemporaryResourceRegistry(pub(crate) HashMap<TemporaryResourceKey, TemporaryResourceState>);

/// Resource that is temporary used by render graph.
/// Because it is temporary, it is not a handle of render graph.
#[derive(Clone)]
pub(crate) enum TemporaryResource {
    Image(Arc<Image>),
    Buffer(Arc<Buffer>),
}

/// TemporaryResourceState itself contains the resource itself.
pub(crate) enum TemporaryResourceState {
    /// Resource that is created but not yet referenced by render graph.
    Inert {
        resource: TemporaryResource,
        access: AccessType,
    },
    /// Resource that is imported as read resource by some pass in the render graph.
    Imported {
        resource: TemporaryResource,
        handle: ExportableGraphResource,
    },
    /// Resource that is imported as write resource by some pass in the render graph.
    Exported {
        resource: TemporaryResource,
        handle: ExportedResourceHandle,
    }
}

impl TemporaryResourceRegistry {
    pub(crate) fn clone_assuming_inert(&self) -> Self {
        Self(
            self.0.iter()
                .map(|(k, v)| match v {
                    TemporaryResourceState::Inert {
                        resource,
                        access,
                    } => (
                        k.clone(),
                        TemporaryResourceState::Inert {
                            resource: resource.clone(),
                            access: *access,
                        },
                    ),
                    TemporaryResourceState::Imported { .. }
                    | TemporaryResourceState::Exported { .. } => {
                        panic!("Trying to clone temporary resource which is not in Inert State!")
                    }
                })
                .collect()
        )
    }
}

/// Render Graph Builder.
/// 
/// It is used to register temporary resources and build passes.
pub struct RenderGraphBuilder {
    device: Arc<Device>,

    render_graph: RenderGraph,
    temporal_resources: TemporaryResourceRegistry,
}

/// Render Graph Builder IS A render graph, but in specific lifetime.
impl std::ops::Deref for RenderGraphBuilder {
    type Target = RenderGraph;

    fn deref(&self) -> &Self::Target {
        &self.render_graph
    }
}

/// Render Graph Builder IS A render graph, but in specific lifetime.
impl std::ops::DerefMut for RenderGraphBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.render_graph
    }
}

impl RenderGraphBuilder {
    pub fn new(device: Arc<Device>, temporal_resources: TemporaryResourceRegistry) -> Self {
        Self {
            device,
            render_graph: RenderGraph::new(),
            temporal_resources,
        }
    }

    pub fn device(&self) -> &Device {
        self.device.as_ref()
    }
}

/// Resource that can be got or created by the render graph.
pub trait GetOrCreateTemporal<Desc: ResourceDesc> {
    /// Get Or Create a temporal resource for user to use.
    /// 
    /// The newly created or fetched resources will be imported into the render graph.
    /// When this is called, the previous frame's Inert resources will be re-imported into RenderGraphBuilder.
    /// When the pass building is complete, it will be exported out of the render graph.
    /// When this frame is complete, those temporary resources will become Inert.
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporaryResourceKey>,
        desc: Desc,
    ) -> anyhow::Result<Handle<<Desc as ResourceDesc>::Resource>>
    where
        Desc: TypeEqualTo<Other = <<Desc as ResourceDesc>::Resource as Resource>::Desc>;
}

impl GetOrCreateTemporal<ImageDesc> for RenderGraphBuilder {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporaryResourceKey>,
        desc: ImageDesc,
    ) -> anyhow::Result<Handle<<ImageDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.0.entry(key.clone()) {
            hash_map::Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                
                match state {
                    TemporaryResourceState::Inert { resource, access } => {
                        // this clone is actually a Arc::clone().
                        let resource = resource.clone();
                    
                        match &resource {
                            TemporaryResource::Image(image) => {
                                let handle = self.render_graph.import(image.clone(), *access);
                                // DO NOT forget to changed the state of this resource
                                *state = TemporaryResourceState::Imported { 
                                    resource, 
                                    handle: ExportableGraphResource::Image(handle.clone_unchecked())
                                };
                                Ok(handle)
                            }
                            TemporaryResource::Buffer(..) => {
                                anyhow::bail!("Required an image resource, but pass in a buffer name! {:?}", key)
                            },
                        }
                    },
                    TemporaryResourceState::Imported { .. } => {
                        Err(anyhow::anyhow!("This temporal resource is already taken by {:?}", key))
                    },
                    TemporaryResourceState::Exported { .. } => {
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
                entry.insert(TemporaryResourceState::Imported {
                    resource: TemporaryResource::Image(resource),
                    handle: ExportableGraphResource::Image(handle.clone_unchecked()),
                });
                Ok(handle)
            }
        }
    }
}

impl GetOrCreateTemporal<BufferDesc> for RenderGraphBuilder {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporaryResourceKey>,
        desc: BufferDesc,
    ) -> anyhow::Result<Handle<<BufferDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.0.entry(key.clone()) {
            hash_map::Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                
                match state {
                    TemporaryResourceState::Inert { resource, access } => {
                        // this clone is actually a Arc::clone().
                        let resource = resource.clone();
                    
                        match &resource {
                            TemporaryResource::Buffer(buffer) => {
                                let handle = self.render_graph.import(buffer.clone(), *access);
                                // DO NOT forget to changed the state of this resource
                                *state = TemporaryResourceState::Imported { 
                                    resource, 
                                    handle: ExportableGraphResource::Buffer(handle.clone_unchecked())
                                };
                                Ok(handle)
                            }
                            TemporaryResource::Image(..) => {
                                anyhow::bail!("Required a buffer resource, but pass in a image name! {:?}", key)
                            },
                        }
                    },
                    TemporaryResourceState::Imported { .. } => {
                        Err(anyhow::anyhow!("This temporal resource is already taken by {:?}", key))
                    },
                    TemporaryResourceState::Exported { .. } => {
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
                entry.insert(TemporaryResourceState::Imported {
                    resource: TemporaryResource::Buffer(resource),
                    handle: ExportableGraphResource::Buffer(handle.clone_unchecked()),
                });
                Ok(handle)
            }
        }
    }
}

/// Just a wrapper over TemporalResourceState.
/// 
/// Use strong type definition to distinguish between resources and exported resources.
pub struct ExportedTemporalResources(pub(crate) TemporaryResourceRegistry);

impl RenderGraphBuilder {
    /// This will be called when after user finishing the preparation of the render graph.
    pub(crate) fn export_all_imported_resources(self) -> (RenderGraph, ExportedTemporalResources) {
        let mut render_graph = self.render_graph;
        let mut registry = self.temporal_resources;

        for state in registry.0.values_mut() {
            match state {
                TemporaryResourceState::Inert { .. } => {
                    // if the resources are not referenced by render graph, it is no need to export them.
                }
                TemporaryResourceState::Imported { resource, handle } => match handle {
                    ExportableGraphResource::Image(handle) => {
                        let handle = render_graph.export(handle.clone_unchecked(), AccessType::Nothing);

                        // change state from imported to exported
                        *state = TemporaryResourceState::Exported {
                            resource: resource.clone(),
                            handle: ExportedResourceHandle::Image(handle),
                        }
                    }
                    ExportableGraphResource::Buffer(handle) => {
                        let handle = render_graph.export(handle.clone_unchecked(), AccessType::Nothing);

                        // change state from imported to exported
                        *state = TemporaryResourceState::Exported {
                            resource: resource.clone(),
                            handle: ExportedResourceHandle::Buffer(handle),
                        }
                    }
                },
                TemporaryResourceState::Exported { .. } => {
                    // there can be any exported resources!
                    unreachable!()
                }
            }
        }

        (render_graph, ExportedTemporalResources(registry))
    }
}

impl ExportedTemporalResources {
    pub(crate) fn consume(self, render_graph: &RetiredRenderGraph) -> TemporaryResourceRegistry {
        let mut registry = self.0;

        for state in registry.0.values_mut() {
            match state {
                TemporaryResourceState::Inert { .. } => {

                },
                TemporaryResourceState::Imported { .. } => {
                    unreachable!()
                },
                TemporaryResourceState::Exported { resource, handle } => {
                    match handle {
                        ExportedResourceHandle::Image(handle) => {
                            *state = TemporaryResourceState::Inert { 
                                resource: resource.clone(),
                                // get the final access type to be next frame's init access type
                                access: render_graph.get_exported_resource_access(*handle),
                            }
                        },
                        ExportedResourceHandle::Buffer(handle) => {
                            *state = TemporaryResourceState::Inert {
                                resource: resource.clone(), 
                                // get the final access type to be next frame's init access type
                                access: render_graph.get_exported_resource_access(*handle),
                            }
                        }
                    }
                }
            }
        }

        registry
    }
}