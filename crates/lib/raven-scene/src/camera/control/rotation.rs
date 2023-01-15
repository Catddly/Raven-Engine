use raven_math::{Quat, Vec3};

use crate::camera::{CameraTransform, CameraControl};

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

    /// Break rotation into a (x, y, z) basis vector
    pub fn to_coordinates(&self) -> (Vec3, Vec3, Vec3) {
        let forward = self.rotation * Vec3::NEG_Z;
        let right = forward.cross(Vec3::Y);
        let up = right.cross(forward);

        (right.normalize(), up.normalize(), forward.normalize())
    }

    pub fn rotate_by(&mut self, delta_rotation: Quat) {
        self.rotation = (self.rotation + delta_rotation).normalize();
    }

    pub fn rotate(&mut self, func: impl FnMut(&mut Quat)) 
    {
        let mut func = func;
        func(&mut self.rotation);
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