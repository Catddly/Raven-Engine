//mod context;
mod util;
pub mod constants;

mod instance;
mod surface;
pub mod physical_device;
mod device;
mod swapchain;

pub mod allocator;
mod buffer;
mod image;

mod shader;
pub mod descriptor;
pub mod pipeline;
pub mod renderpass;

pub mod barrier;
mod command;
mod error;

pub use instance::Instance;
pub use surface::Surface;
pub use physical_device::{PhysicalDevice, QueueFamily};
pub use device::Device;
pub use swapchain::{Swapchain, SwapchainImage};
pub use buffer::{Buffer, BufferDesc};
pub use image::{Image, ImageDesc, ImageType, ImageViewDesc};

pub use shader::{ShaderSource, ShaderBinary, ShaderBinaryStage, PipelineShaderStage, PipelineShaderDesc};
pub use pipeline::{RasterPipelineDesc, ComputePipelineDesc, RasterPipeline, ComputePipeline};
pub use renderpass::{RenderPass, RenderPassDesc, RenderPassAttachmentDesc};

pub use barrier::{AccessType, ImageBarrier, BufferBarrier};

pub use command::CommandBuffer;
pub use error::RHIError;

pub use util::debug;
pub use util::platform;
pub use util::utility;