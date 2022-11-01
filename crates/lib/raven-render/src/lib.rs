extern crate log as glog; // to avoid name collision with my log module

mod renderer;

pub use renderer::mesh_renderer::{MeshRenderer, MeshRasterScheme, MeshShadingContext};
pub use renderer::sky_renderer::{SkyRenderer};
pub use renderer::ibl_renderer::{IblRenderer};