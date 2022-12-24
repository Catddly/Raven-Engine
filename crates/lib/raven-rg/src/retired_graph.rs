use raven_rhi::{
    backend::AccessType,
};

use crate::{
    resource::Resource,
    graph_resource::ExportedHandle,
    compiled_graph::{RegisteredResource, GraphPreparedResource},
    transient_resource_cache::TransientResourceCache,
};

pub(crate) struct RetiredRenderGraph {
    pub(crate) registered_resources: Vec<RegisteredResource>,
}

impl RetiredRenderGraph {
    pub fn get_exported_resource_access<Res: Resource>(
        &self,
        handle: ExportedHandle<Res>
    ) -> AccessType {
        self.registered_resources[handle.handle.id as usize].get_current_access()
    }

    /// Release all the resources that is created by the render graph.
    /// THese resources might be used in next frame, use cache to avoid frequently create and destroy resources.
    pub fn release_owned_resources(
        self,
        cache: &mut TransientResourceCache,
    ) {
        for res in self.registered_resources.into_iter() {
            match res.resource {
                GraphPreparedResource::CreatedImage(image) => {
                    cache.store_image(image);
                },
                GraphPreparedResource::CreatedBuffer(buffer) => {
                    cache.store_buffer(buffer);
                }
                #[cfg(feature = "gpu_ray_tracing")]
                GraphPreparedResource::ImportedRayTracingAccelStruct(_) | 
                GraphPreparedResource::ImportedImage(_) |
                GraphPreparedResource::ImportedBuffer(_) => {}
                #[cfg(not(feature = "gpu_ray_tracing"))]
                GraphPreparedResource::ImportedImage(_) |
                GraphPreparedResource::ImportedBuffer(_) => {}

                GraphPreparedResource::Delayed(_) => panic!("Try to finish render graph while still some resources is in GraphPreparedResource::Delayed state."),
            }
        }
    }
}