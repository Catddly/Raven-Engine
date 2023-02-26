// Raven Engine exposed APIs
pub use super::user::App;

pub use crate::raven_main;

// core module
pub mod core {
    pub use crate::core::{
        CoreApi,
        get,
    };
}

// asset module
pub mod asset {
    pub use crate::asset::{
        AssetApi,
        AssetLoadDesc, AssetHandle,
        AssetType, AsConcreteRawAsset, AsConcreteAsset,
        get,
    };
}

// input module
pub mod input {
    pub use crate::input::{
        InputApi, InputBinding,
        KeyCode, MouseButton, VirtualKeyCode,
        get,
    };
}

// render module
pub mod render {
    pub use crate::render::{
        RenderApi, RhiConfig,
        LightFrameConstants, FrameConstants,
        MeshHandle, MeshInstanceHandle, RgHandle,
        get,
    };
}

// math module
pub mod math {
    pub use raven_facade::math::{
        *
    };
}

// scene
pub mod scene {
    pub use raven_facade::scene::{
        camera
    };
}