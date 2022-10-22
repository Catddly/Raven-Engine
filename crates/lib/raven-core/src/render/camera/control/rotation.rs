use glam::{Quat};

use crate::render::camera::{CameraTransform, CameraControl};

// TODO: this will have problems on overwriting user's default camera position
pub struct Rotation {
    rotation: Quat,
}

impl Rotation {
    pub fn new() -> Self {
        Self {
            rotation: Default::default(),
        }
    }

    pub fn rotate_to(&mut self, dst_rotation: Quat) {
        self.rotation = dst_rotation.normalize();
    }

    pub fn rotate_by(&mut self, delta_rotation: Quat) {
        self.rotation = (self.rotation + delta_rotation).normalize();
    }
}

impl CameraControl for Rotation {
    fn update(&self, prev_trans: CameraTransform) -> CameraTransform {
        CameraTransform {
            position: prev_trans.position,
            rotation: self.rotation,
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self        
    }
}