use std::sync::Arc;
use std::cell::Cell;

use raven_rhi::{
    backend::{Image, Buffer, AccessType},
    pipeline_cache::{RasterPipelineHandle, ComputePipelineHandle}, dynamic_buffer::DynamicBuffer,
};
#[cfg(feature = "gpu_ray_tracing")]
use raven_rhi::{
    backend::RayTracingAccelerationStructure,
    pipeline_cache::{RayTracingPipelineHandle}
};

use crate::{
    graph::{RenderGraph, AnalyzedResourceInfos, ResourceUsage},
    graph_resource::{GraphResource, GraphResourceDesc, GraphResourceImportedData},
    executing_graph::ExecutingRenderGraph,
    transient_resource_cache::TransientResourceCache, graph_executor::{ExecutionParams},
};

/// Pipeline handles to the pipeline cache.
pub struct RenderGraphPipelineHandles {
    pub raster_pipeline_handles: Vec<RasterPipelineHandle>,
    pub compute_pipeline_handles: Vec<ComputePipelineHandle>,
    #[cfg(feature = "gpu_ray_tracing")]
    pub ray_tracing_pipeline_handles: Vec<RayTracingPipelineHandle>,
}

pub(crate) struct CompiledRenderGraph {
    pub(crate) render_graph: RenderGraph,
    pub(crate) resource_infos: AnalyzedResourceInfos,

    pub(crate) pipelines: RenderGraphPipelineHandles,
}

pub(crate) enum GraphPreparedResource {
    CreatedImage(Image),
    ImportedImage(Arc<Image>),
    CreatedBuffer(Buffer),
    ImportedBuffer(Arc<Buffer>),

    #[cfg(feature = "gpu_ray_tracing")]
    ImportedRayTracingAccelStruct(Arc<RayTracingAccelerationStructure>),

    Delayed(GraphResource),
}

impl GraphPreparedResource {
    pub fn borrow(&self) -> GraphPreparedResourceRef {
        match &self {
            GraphPreparedResource::CreatedImage(image) => GraphPreparedResourceRef::Image(image),
            GraphPreparedResource::ImportedImage(image) => GraphPreparedResourceRef::Image(&*image),

            GraphPreparedResource::CreatedBuffer(buffer) => GraphPreparedResourceRef::Buffer(buffer),
            GraphPreparedResource::ImportedBuffer(buffer) => GraphPreparedResourceRef::Buffer(&*buffer),

            #[cfg(feature = "gpu_ray_tracing")]
            GraphPreparedResource::ImportedRayTracingAccelStruct(accel_struct) => 
                GraphPreparedResourceRef::RayTracingAccelStruct(&*accel_struct),

            GraphPreparedResource::Delayed(_) => panic!("Can not borrow GraphPreparedResource::Delayed resource, it doesn't exist!"),
        }
    }
}

// used to borrow inner resource from GraphPreparedResource, and flatten out the differences of Created or Imported.
pub(crate) enum GraphPreparedResourceRef<'a> {
    Image(&'a Image),
    Buffer(&'a Buffer),

    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct(&'a RayTracingAccelerationStructure),
}

pub struct RegisteredResource {
    pub(crate) resource: GraphPreparedResource,
    access: Cell<AccessType>,
}

impl RegisteredResource {
    pub fn get_current_access(&self) -> AccessType {
        self.access.get()
    }

    #[inline]
    pub fn transition_to(&self, dst_access: AccessType) {
        self.access.set(dst_access);
    }
}

impl CompiledRenderGraph {
    /// Gather or create all the resources.
    #[must_use]
    pub(crate) fn prepare_execute<'exec, 'dynamic> (
        self,
        execution_params: ExecutionParams<'exec>, 
        cache: &mut TransientResourceCache,
        global_dynamic_constants_buffer: &'dynamic mut DynamicBuffer,
    ) -> ExecutingRenderGraph<'exec, 'dynamic> {
        let device = execution_params.device;

        let registered_resources = self.render_graph.resources.iter()
            .enumerate()
            .map(|(idx, resource)| {
                match resource {
                    GraphResource::Created(created) => {
                        match created.desc {
                            GraphResourceDesc::Image(mut desc) => {
                                if let ResourceUsage::Image(usage) = self.resource_infos.resource_usages[idx] {
                                    desc.usage = usage;
                                } else {
                                    panic!("Expect image description, but not found in the analyzed resource infos!");
                                }

                                // get image from the cache of the last frame
                                let image = if let Some(image) = cache.get_image(&desc) {
                                    image
                                } else {
                                    device.create_image(desc.clone(), None).unwrap()
                                };

                                RegisteredResource {
                                    // this resource is owned by the render graph, it doesn't matter what its access type is at first.
                                    access: Cell::new(AccessType::Nothing),
                                    resource: GraphPreparedResource::CreatedImage(image),
                                }
                            }
                            GraphResourceDesc::Buffer(mut desc) => {
                                if let ResourceUsage::Buffer(usage) = self.resource_infos.resource_usages[idx] {
                                    desc.usage = usage;
                                } else {
                                    panic!("Expect buffer description, but not found in the analyzed resource infos!");
                                }

                                let buffer = if let Some(buffer) = cache.get_buffer(&desc) {
                                    buffer
                                } else {
                                    device.create_buffer(desc.clone(), "rg_created_buffer").unwrap()
                                };

                                RegisteredResource {
                                    // this resource is owned by the render graph, it doesn't matter what its access type is at first.
                                    access: Cell::new(AccessType::Nothing),
                                    resource: GraphPreparedResource::CreatedBuffer(buffer),
                                }
                            }
                            // TODO: we only import ray tracing acceleration structure now!
                            #[cfg(feature = "gpu_ray_tracing")]
                            GraphResourceDesc::RayTracingAccelStruct(_) => unimplemented!()
                        }
                    },
                    GraphResource::Imported(imported) => {
                        match imported {
                            GraphResourceImportedData::Image{ raw, access } => {
                                RegisteredResource {
                                    access: Cell::new(*access),
                                    resource: GraphPreparedResource::ImportedImage(raw.clone()),
                                }
                            }
                            GraphResourceImportedData::Buffer{ raw, access } => {
                                RegisteredResource {
                                    access: Cell::new(*access),
                                    resource: GraphPreparedResource::ImportedBuffer(raw.clone()),
                                }
                            }
                            #[cfg(feature = "gpu_ray_tracing")]
                            GraphResourceImportedData::RayTracingAccelStruct{ raw, access } => {
                                RegisteredResource {
                                    access: Cell::new(*access),
                                    resource: GraphPreparedResource::ImportedRayTracingAccelStruct(raw.clone()),
                                }
                            }
                            GraphResourceImportedData::SwapchainImage => {
                                RegisteredResource {
                                    access: Cell::new(AccessType::ComputeShaderWrite),
                                    resource: GraphPreparedResource::Delayed(resource.clone()),
                                }
                            }
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        ExecutingRenderGraph {
            execution_params,

            global_dynamic_buffer: global_dynamic_constants_buffer,

            passes: self.render_graph.passes.into(),
            native_resources: self.render_graph.resources,
            registered_resources,
            exported_resources: self.render_graph.exported_resources,

            pipelines: self.pipelines,
        }
    }
}