use raven_rhi::backend::{Buffer, BufferDesc, Image, ImageDesc};
#[cfg(feature = "gpu_ray_tracing")]
use raven_rhi::backend::{RayTracingAccelerationStructure};

use super::graph_resource::GraphResourceDesc;

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Copy, Clone, Debug)]
pub struct RayTracingAccelStructDesc;

/// Used this trait to transmute one type to other type.
pub trait TypeEqualTo {
    type Other;
    
    fn same(v: Self) -> Self::Other;
}

impl<T: Sized> TypeEqualTo for T {
    type Other = T;

    fn same(v: Self) -> Self::Other {
        v
    }
}

/// Any outer render resource.
pub trait Resource {
    type Desc: ResourceDesc;
}

impl Resource for Buffer {
    type Desc = BufferDesc;
}

impl Resource for Image {
    type Desc = ImageDesc;
}

#[cfg(feature = "gpu_ray_tracing")]
impl Resource for RayTracingAccelerationStructure {
    type Desc = RayTracingAccelStructDesc;
}

/// Any outer resource description.
pub trait ResourceDesc: Clone + Into<GraphResourceDesc> + std::fmt::Debug {
    type Resource: Resource;
}

impl ResourceDesc for BufferDesc {
    type Resource = Buffer;
}

impl ResourceDesc for ImageDesc {
    type Resource = Image;
}

#[cfg(feature = "gpu_ray_tracing")]
impl ResourceDesc for RayTracingAccelStructDesc {
    type Resource = RayTracingAccelerationStructure;
}

impl Into<GraphResourceDesc> for ImageDesc {
    fn into(self) -> GraphResourceDesc {
        GraphResourceDesc::Image(self)
    }
}

impl Into<GraphResourceDesc> for BufferDesc {
    fn into(self) -> GraphResourceDesc {
        GraphResourceDesc::Buffer(self)
    }
}

#[cfg(feature = "gpu_ray_tracing")]
impl Into<GraphResourceDesc> for RayTracingAccelStructDesc {
    fn into(self) -> GraphResourceDesc {
        GraphResourceDesc::RayTracingAccelStruct(self)
    }
}

/// Shader Resource View.
pub struct Srv;
/// Unordered Access View.
pub struct Uav;
/// Render Target.
pub struct Rt;

/// Used as compile-time marker to determine a resource's view type.
pub trait ResourceView {
    const IS_WRITABLE: bool;
}

impl ResourceView for Srv {
    const IS_WRITABLE: bool = false; 
}

impl ResourceView for Uav {
    const IS_WRITABLE: bool = true; 
}

impl ResourceView for Rt {
    const IS_WRITABLE: bool = true; 
}