use std::sync::Arc;

use raven_rhi::backend::{Buffer, BufferDesc, Image, ImageDesc};

use super::graph_resource::GraphResourceDesc;

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

/// Shader Resource View.
pub struct SRV;
/// Unordered Access View.
pub struct UAV;
/// Render Target.
pub struct RT;

/// Used as compiled time marker to determine a resource's view type.
pub trait ResourceView {
    const IS_WRITABLE: bool;
}

impl ResourceView for SRV {
    const IS_WRITABLE: bool = false; 
}

impl ResourceView for UAV {
    const IS_WRITABLE: bool = true; 
}

impl ResourceView for RT {
    const IS_WRITABLE: bool = true; 
}