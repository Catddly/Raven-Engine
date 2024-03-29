mod graph_builder;
mod graph;
mod compiled_graph;
mod executing_graph;
mod retired_graph;

mod resource;
mod graph_resource;

mod pass;
mod pass_context;

mod graph_executor;
mod transient_resource_cache;

mod helper;

pub use graph_resource::Handle as RgHandle;
pub use graph_builder::{RenderGraphBuilder, GetOrCreateTemporal};
pub use graph_executor::{GraphExecutor, FrameConstants, LightFrameConstants};
pub use pass_context::{
    IntoPipelineDescriptorBindings, RenderGraphPassBinding, RenderGraphPassBindable, PassContext,
    BoundRasterPipeline, BoundComputePipeline
};
#[cfg(feature = "gpu_ray_tracing")]
pub use pass_context::{BoundRayTracingPipeline};

pub use helper::image_clear;

// pub use global_bindless_descriptor::create_engine_global_bindless_descriptor_set;

extern crate log as glog;