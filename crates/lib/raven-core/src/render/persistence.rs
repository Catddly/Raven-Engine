use glam::{Vec3, Quat};

pub trait IsStatesChanged {
    fn is_states_changed(&self, _: &Self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct CameraPersistState {
    pub position: Vec3,
    pub rotation: Quat,
}

impl IsStatesChanged for CameraPersistState {
    fn is_states_changed(&self, other: &Self) -> bool {
        !self.position.abs_diff_eq(other.position, 1e-5) ||
        !self.rotation.abs_diff_eq(other.rotation, 1e-5)
    }
}

impl Default for CameraPersistState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistStates {
    pub camera: CameraPersistState,
}

impl PersistStates {
    pub fn new() -> Self {
        Self {
            camera: Default::default(),
        }
    }
}

impl IsStatesChanged for PersistStates {
    fn is_states_changed(&self, other: &Self) -> bool {
        self.camera.is_states_changed(&other.camera)
    }
}