use glam::Vec3;

use crate::render::camera::{CameraTransform, CameraControl};

pub struct Position {
    position: Vec3,
}

impl Position {
    pub fn new() -> Self {
        Self {
            position: Default::default(),
        }
    }

    pub fn move_to(&mut self, dst_pos: Vec3) {
        self.position = dst_pos;
    }

    pub fn move_by(&mut self, delta_pos: Vec3) {
        self.position += delta_pos;
    }
}

impl CameraControl for Position {
    fn update(&self, prev_trans: CameraTransform) -> CameraTransform {
        CameraTransform {
            position: self.position,
            rotation: prev_trans.rotation,
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self        
    }
}