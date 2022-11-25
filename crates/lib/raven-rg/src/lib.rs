mod graph_builder;
mod graph;
mod compiled_graph;
mod executing_graph;
mod retired_graph;

mod resource;
mod graph_resource;

mod pass;
mod pass_context;

mod executor;
mod transient_resource_cache;

mod helper;

mod global_bindless_descriptor;  // set 1
mod global_constants_descriptor; // set 2

pub use graph_resource::Handle as RgHandle;
pub use graph_builder::{RenderGraphBuilder, GetOrCreateTemporal};
pub use executor::{Executor, FrameConstants};
pub use pass_context::{IntoPipelineDescriptorBindings, RenderGraphPassBinding, RenderGraphPassBindable};

pub use helper::image_clear;

pub use global_bindless_descriptor::create_engine_global_bindless_descriptor_set;

extern crate log as glog;