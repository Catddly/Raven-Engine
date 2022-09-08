#[cfg(debug_assertions)]
pub(crate) const ENABLE_DEBUG: bool = true;
#[cfg(not(debug_assertions))]
pub(crate) const ENABLE_DEBUG: bool = false;

/// Required vulkan validation layer name
pub(crate) const REQUIRED_VALIDATION_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

#[cfg(feature = "ray_tracing")]
pub(crate) const ENABLE_GPU_RAY_TRACING : bool = true;
#[cfg(not(feature = "ray_tracing"))]
pub(crate) const ENABLE_GPU_RAY_TRACING : bool = false;