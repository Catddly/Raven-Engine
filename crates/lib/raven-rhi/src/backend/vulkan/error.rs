use raven_core::thiserror::Error;

#[derive(Debug, Error)]
pub enum RHIError {
    #[error("Allocation failed for {name:?}: {error:?}")]
    AllocationFailure {
        name: String,
        error: gpu_allocator::AllocationError,
    },

    #[error("Vulkan error: {err:?}")]
    Vulkan { err: ash::vk::Result },

    // #[error("Shader Failed to compile with: {error:?}")]
    // ShaderCompilation { error: LazyEvalError },

    #[error("Vulkan framebuffer is invalid, need to reconstruct!")]
    FramebufferInvalid,

    #[error("Vulkan failed on acquiring next image: {err:?}")]
    AcquiredImageFailed { err: ash::vk::Result },
}

impl From<ash::vk::Result> for RHIError {
    fn from(err: ash::vk::Result) -> Self {
        Self::Vulkan {
            err,
        }
    }
}