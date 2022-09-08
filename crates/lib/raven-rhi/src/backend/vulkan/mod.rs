//mod context;
mod util;
mod constants;

mod instance;
mod surface;
pub mod physical_device;
mod device;
mod swapchain;

mod allocator;
mod buffer;
mod image;

mod shader;
pub mod descriptor;
mod pipeline;
mod render_pass;

pub mod barrier;
mod error;

pub use instance::Instance;
pub use surface::Surface;
pub use physical_device::{PhysicalDevice};
pub use device::Device;
pub use swapchain::Swapchain;
pub use buffer::{Buffer, BufferDesc};
pub use image::{Image, ImageDesc, ImageType, ImageViewDesc};

pub use shader::{ShaderSource, ShaderBinary, ShaderBinaryStage, PipelineShaderStage, PipelineShaderDesc};
pub use pipeline::{RasterPipelineDesc, ComputePipelineDesc, RasterPipeline, ComputePipeline};
pub use render_pass::RenderPass;

pub use error::RHIError;

pub use util::debug;
pub use util::platform;
pub use util::utility;