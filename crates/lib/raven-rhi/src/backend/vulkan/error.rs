use raven_core::thiserror::Error;
//use turbosloth::lazy::LazyEvalError;

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
}

impl From<ash::vk::Result> for RHIError {
    fn from(err: ash::vk::Result) -> Self {
        Self::Vulkan {
            err,
        }
    }
}