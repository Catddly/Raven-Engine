use glam::{Vec3, Quat, Vec2, Mat3};

use crate::render::camera::{CameraController, CameraControllerBuilder, Camera};
use super::super::control::{CamCtrlPosition, CamCtrlRotation};

/// This is just a simple wrapper on CameraController.
/// Provide user some common camera controller behaviors.
pub struct FirstPersonController {
    controller: CameraController,

    move_speed: f32,
    view_speed: f32,
}

impl std::ops::Deref for FirstPersonController {
    type Target = CameraController;
    
    fn deref(&self) -> &Self::Target {
        &self.controller
    }
}

impl std::ops::DerefMut for FirstPersonController {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.controller
    }
}

impl FirstPersonController {
    pub fn new(pos: Vec3, rotation: Quat) -> Self {
        let mut pos_ctrl = CamCtrlPosition::new();
        let mut rot_ctrl = CamCtrlRotation::new();
        pos_ctrl.move_to(pos);
        rot_ctrl.rotate_to(rotation);

        Self {
            controller: CameraControllerBuilder::new()
                .with(pos_ctrl)
                .with(rot_ctrl)
                .build(),
            move_speed: 0.005,
            view_speed: 1.0,
        }
    }

    pub fn change_to(&mut self, controller: impl Into<CameraController>) {
        self.controller = controller.into();
    }

    // TODO: consider using event system or callback function?
    pub fn update(&mut self, 
        camera: &mut Camera, mouse_delta: Vec2, is_left_mouse_holding: bool,
        input_walk: f32, input_strafe: f32, input_lift: f32
    ) {
        let (mut right, mut up, mut forward) = self.controller.get_control_mut::<CamCtrlRotation>().to_coordinates();

        if is_left_mouse_holding {
            (forward, right) = if mouse_delta.x != 0.0 {
                let horizon_rot = Quat::from_axis_angle(Vec3::NEG_Y, mouse_delta.x * self.view_speed);
                (horizon_rot.mul_vec3(forward).normalize(), horizon_rot.mul_vec3(right).normalize())
            } else {
                (forward, right)
            };

            (forward, up) = if mouse_delta.y != 0.0 {
                let vertical_rot = Quat::from_axis_angle(-right, mouse_delta.y * self.view_speed * camera.lens.aspect_ratio);
                let new_forward = vertical_rot.mul_vec3(forward).normalize();

                let dot = new_forward.dot(Vec3::Y);
                if dot >= 0.9999 || dot <= -0.9999 {
                    (forward, up)
                } else {
                    (new_forward, vertical_rot.mul_vec3(up).normalize())
                }
            } else {
                (forward, up)
            };

            let rotation_mat = Mat3::from_cols_array_2d(&[
                [right.x,     right.y,    right.z  ],
                [up.x,        up.y,       up.z     ],
                [-forward.x, -forward.y, -forward.z],
            ]);
            self.controller.get_control_mut::<CamCtrlRotation>().rotate_to(Quat::from_mat3(&rotation_mat).normalize());
            
        }

        let delta_pos = 
            input_strafe * self.move_speed * right +
            input_lift   * self.move_speed * up +
            input_walk   * self.move_speed * forward;

        self.controller.get_control_mut::<CamCtrlPosition>().move_by(delta_pos);
        self.controller.update(camera);
    }
}