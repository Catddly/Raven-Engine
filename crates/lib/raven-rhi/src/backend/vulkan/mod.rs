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
mod sampler;

mod shader;
pub mod descriptor;
pub mod pipeline;
pub mod renderpass;

#[cfg(feature = "gpu_ray_tracing")]
mod ray_tracing;

pub mod barrier;
mod command;
mod error;

pub use instance::Instance;
pub use surface::Surface;
pub use physical_device::{PhysicalDevice, QueueFamily};
pub use device::Device;
pub use swapchain::{Swapchain, SwapchainImage};
pub use buffer::{Buffer, BufferDesc};
pub use image::{Image, ImageDesc, ImageSubResource, ImageType, ImageViewDesc};
pub use sampler::{SamplerDesc};

pub use shader::{ShaderSource, ShaderBinary, ShaderBinaryStage, PipelineShaderStage, PipelineShaderDesc};
pub use pipeline::{
    RasterPipelineDesc, ComputePipelineDesc, RasterPipeline, ComputePipeline,
    RasterPipelinePrimitiveTopology, RasterPipelineCullMode
};
#[cfg(feature = "gpu_ray_tracing")]
pub use pipeline::{RayTracingPipelineDesc, RayTracingPipeline};
pub use renderpass::{RenderPass, RenderPassDesc, RenderPassAttachmentDesc};

pub use barrier::{AccessType, ImageBarrier, BufferBarrier};

#[cfg(feature = "gpu_ray_tracing")]
pub use ray_tracing::{
    RayTracingAccelerationStructure, RayTracingAccelerationScratchBuffer, 
    RayTracingBlasBuildDesc, RayTracingTlasBuildDesc,
    RayTracingGeometry, RayTracingGeometryType, RayTracingSubGeometry,
    RayTracingBlasInstance,
};

pub use command::CommandBuffer;
pub use error::RhiError;

pub use util::debug;
pub use util::platform;
pub use util::utility;