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

#[derive(Default)]
pub struct TemporalResourceRegistry(pub(crate) HashMap<TemporalResourceKey, TemporalResourceState>);

#[derive(Clone)]
pub(crate) enum TemporalResource {
    Image(Arc<Image>),
    Buffer(Arc<Buffer>),
}

/// TemporaryResourceState itself contains the resource itself.
pub(crate) enum TemporalResourceState {
    /// Resource that is created but not yet referenced by render graph.
    Inert {
        resource: TemporalResource,
        access: AccessType,
    },
    Imported {
        resource: TemporalResource,
        handle: ExportableGraphResource,
    },
    Exported {
        resource: TemporalResource,
        handle: ExportedResourceHandle,
    }
}

impl TemporalResourceRegistry {
    pub(crate) fn clone_assuming_inert(&self) -> Self {
        Self(
            self.0.iter()
                .map(|(k, v)| match v {
                    TemporalResourceState::Inert {
                        resource,
                        access,
                    } => (
                        k.clone(),
                        TemporalResourceState::Inert {
                            resource: resource.clone(),
                            access: *access,
                        },
                    ),
                    TemporalResourceState::Imported { .. }
                    | TemporalResourceState::Exported { .. } => {
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
    temporal_resources: TemporalResourceRegistry,
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
    pub fn new(device: Arc<Device>, temporal_resources: TemporalResourceRegistry) -> Self {
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
    /// When the pass building is complete, it will be exported out of the render graph and can be used by the graph.
    /// When this frame is complete, those temporary resources will become Inert.
    /// 
    /// # Note
    /// 
    /// RayTracingAccelerationStructure has not been added to the TemporalResource yet,
    /// so use this function on RayTracingAccelStructDesc is undefine.
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: Desc,
    ) -> anyhow::Result<Handle<<Desc as ResourceDesc>::Resource>>
    where
        Desc: TypeEqualTo<Other = <<Desc as ResourceDesc>::Resource as Resource>::Desc>;
}

impl GetOrCreateTemporal<ImageDesc> for RenderGraphBuilder {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: ImageDesc,
    ) -> anyhow::Result<Handle<<ImageDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.0.entry(key.clone()) {
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
                        .create_image(desc, None)
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

impl GetOrCreateTemporal<BufferDesc> for RenderGraphBuilder {
    fn get_or_create_temporal(
        &mut self,
        name: impl Into<TemporalResourceKey>,
        desc: BufferDesc,
    ) -> anyhow::Result<Handle<<BufferDesc as ResourceDesc>::Resource>> {
        let key = name.into();

        match self.temporal_resources.0.entry(key.clone()) {
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
                            }
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
/// 
/// Use strong type definition to distinguish between resources and exported resources.
pub struct ExportedTemporalResources(pub(crate) TemporalResourceRegistry);

impl RenderGraphBuilder {
    /// This will be called when after user finishing the preparation of the render graph.
    pub(crate) fn build(self) -> (RenderGraph, ExportedTemporalResources) {
        let mut render_graph = self.render_graph;
        let mut registry = self.temporal_resources;

        for state in registry.0.values_mut() {
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
                    #[cfg(feature = "gpu_ray_tracing")]
                    ExportableGraphResource::RayTracingAccelStruct(handle) => {
                        let handle = render_graph.export(handle.clone_unchecked(), AccessType::Nothing);

                        // change state from imported to exported
                        *state = TemporalResourceState::Exported {
                            resource: resource.clone(),
                            handle: ExportedResourceHandle::RayTracingAccelStruct(handle),
                        }
                    }
                }
                TemporalResourceState::Exported { .. } => {
                    // there can't be any exported resources!
                    unreachable!()
                }
            }
        }

        (render_graph, ExportedTemporalResources(registry))
    }
}

impl ExportedTemporalResources {
    pub(crate) fn consume(self, render_graph: &RetiredRenderGraph) -> TemporalResourceRegistry {
        let mut registry = self.0;

        for state in registry.0.values_mut() {
            match state {
                TemporalResourceState::Inert { .. } => {

                },
                TemporalResourceState::Imported { .. } => {
                    unreachable!()
                },
                TemporalResourceState::Exported { resource, handle } => {
                    match handle {
                        ExportedResourceHandle::Image(handle) => {
                            *state = TemporalResourceState::Inert { 
                                resource: resource.clone(),
                                // get the final access type to be next frame's init access type
                                access: render_graph.get_exported_resource_access(*handle),
                            }
                        }
                        ExportedResourceHandle::Buffer(handle) => {
                            *state = TemporalResourceState::Inert {
                                resource: resource.clone(), 
                                // get the final access type to be next frame's init access type
                                access: render_graph.get_exported_resource_access(*handle),
                            }
                        }
                        #[cfg(feature = "gpu_ray_tracing")]
                        ExportedResourceHandle::RayTracingAccelStruct(handle) => {
                            *state = TemporalResourceState::Inert {
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