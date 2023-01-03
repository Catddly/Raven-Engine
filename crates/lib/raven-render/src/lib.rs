extern crate log as glog; // to avoid name collision with my log module

mod renderer;

pub use renderer::mesh_renderer::{MeshRenderer, MeshRasterScheme, MeshShadingContext};
pub use renderer::light_renderer::{LightRenderer};
pub use renderer::sky_renderer::{SkyRenderer};
pub use renderer::ibl_renderer::{IblRenderer};

pub use renderer::debug_renderer::{DebugRenderer};

mod world_renderer;

pub use world_renderer::{WorldRenderer, RenderMode};