extern crate log as glog; // to avoid name collision with my log module

mod renderer;

mod global_bindless_descriptor;  // set 1

pub use renderer::mesh_renderer::{MeshRenderer, MeshRasterScheme, MeshShadingContext};