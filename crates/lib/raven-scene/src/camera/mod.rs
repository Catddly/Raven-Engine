pub mod control;
pub mod controller;

use std::any::Any;

use raven_math::{Vec3, Quat, Mat4, Vec4};

use raven_math::AABB;

// TODO: move this to config
const CAMERA_MAX_SHADOW_Z: f32 = 500.0;

#[derive(Copy, Clone)]
pub struct CameraTransform {
    position: Vec3,
    rotation: Quat,
}

impl From<CameraBody> for CameraTransform {
    fn from(body: CameraBody) -> Self {
        Self {
            position: body.position,
            rotation: body.rotation,
        }
    }
}

impl CameraTransform {
    pub const IDENTITY: Self = CameraTransform {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
}

#[derive(Copy, Clone)]
pub struct CameraBody {
    pub position: Vec3,
    /// Quaternion here must be normalized
    pub rotation: Quat,
}

impl Default for CameraBody {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl From<CameraTransform> for CameraBody {
    fn from(xform: CameraTransform) -> Self {
        Self {
            position: xform.position,
            rotation: xform.rotation,
        }
    }
}

// TODO: support different projections
pub struct CameraLens {
    pub aspect_ratio: f32,
    pub fov_vertical_degrees: f32,
    pub near_plane: f32,
}

impl Default for CameraLens {
    fn default() -> Self {
        Self {
            aspect_ratio: 1.0,
            fov_vertical_degrees: 55.0,
            near_plane: 0.01, // 0.01 m
        }
    }
}

#[derive(Default)]
pub struct Camera {
    pub body: CameraBody,
    pub lens: CameraLens,
}

impl Camera {
    pub fn builder() -> CameraBuilder {
        CameraBuilder::new()
    }

    pub fn with_body(&mut self, body: CameraBody) {
        self.body = body;
    }

    pub fn with_lens(&mut self, lens: CameraLens) {
        self.lens = lens;
    }

    pub fn get_camera_render_data(&self) -> CameraFrameConstants {
        // rotation first, and then translation
        let view_to_world = {
            let translation = Mat4::from_translation(self.body.position);
            translation * Mat4::from_quat(self.body.rotation)
        };

        // image we transform the camera matrix back to the world's center
        let world_to_view = {
            let inv_translation = Mat4::from_translation(-self.body.position);
            Mat4::from_quat(self.body.rotation.conjugate()) * inv_translation
        };

        // TODO: add math derivation to Doc
        let view_to_clip = Mat4::perspective_infinite_reverse_rh(
            self.lens.fov_vertical_degrees.to_radians(),
            self.lens.aspect_ratio,
            self.lens.near_plane
        );

        // use row-reduction to compute inverse matrix (faster than calling inverse())
        let clip_to_view = Mat4::from_cols(
            Vec4::new(view_to_clip.col(0).x.recip(), 0.0, 0.0, 0.0),
            Vec4::new(0.0, view_to_clip.col(1).y.recip(), 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, self.lens.near_plane.recip()),
            Vec4::new(0.0, 0.0, -1.0, 0.0)
        );

        CameraFrameConstants {
            world_to_view,
            view_to_world,
            view_to_clip,
            clip_to_view,
        }
    }

    pub fn get_camera_frustum_aabb(&self) -> AABB {
        // TODO: cache matrix data
        let view_to_world = {
            let translation = Mat4::from_translation(self.body.position);
            translation * Mat4::from_quat(self.body.rotation)
        };

        // TODO: add math derivation to Doc
        let view_to_clip = Mat4::perspective_rh(
            self.lens.fov_vertical_degrees.to_radians(),
            self.lens.aspect_ratio,
            // reverse z
            CAMERA_MAX_SHADOW_Z,
            self.lens.near_plane,
        );
        let clip_to_view = view_to_clip.inverse();
        let frustum_aabb = Self::get_camera_frustum_aabb_impl(view_to_world * clip_to_view);

        frustum_aabb
    }

    pub fn get_camera_frustum_line_lists(&self) -> Vec<Vec3> {
        let mut res = Vec::with_capacity(24);

        let view_to_world = {
            let translation = Mat4::from_translation(self.body.position);
            translation * Mat4::from_quat(self.body.rotation)
        };

        // let view_to_clip = Mat4::perspective_rh(
        //     self.lens.fov_vertical_degrees.to_radians(),
        //     self.lens.aspect_ratio,
        //     0.1,
        //     10.0
        // );
        let view_to_clip = Mat4::perspective_rh(
            self.lens.fov_vertical_degrees.to_radians(),
            self.lens.aspect_ratio,
            // reverse z
            CAMERA_MAX_SHADOW_Z,
            self.lens.near_plane,
        );
        // TODO: use row-reduction to compute inverse matrix (faster than calling inverse())
        let clip_to_view = view_to_clip.inverse();
        let clip_to_world = view_to_world * clip_to_view;

        let clip_space_points = [
            // line [1]
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0,  1.0, 1.0),
            // line [2]
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new( 1.0, -1.0, 1.0),
            // line [3]
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0, -1.0, 0.0),

            // line [4]
            Vec3::new( 1.0,  1.0, 0.0),
            Vec3::new(-1.0,  1.0, 0.0),
            // line [5]
            Vec3::new( 1.0,  1.0, 0.0),
            Vec3::new( 1.0, -1.0, 0.0),
            // line [6]
            Vec3::new( 1.0,  1.0, 0.0),
            Vec3::new( 1.0,  1.0, 1.0),

            // line [7]
            Vec3::new( 1.0, -1.0, 1.0),
            Vec3::new( 1.0,  1.0, 1.0),
            // line [8]
            Vec3::new( 1.0, -1.0, 1.0),
            Vec3::new( 1.0,  1.0, 0.0),

            // line [9]
            Vec3::new(-1.0,  1.0, 0.0),
            Vec3::new(-1.0,  1.0, 1.0),
            // line [10]
            Vec3::new(-1.0,  1.0, 0.0),
            Vec3::new(-1.0, -1.0, 0.0),

            // line [11]
            Vec3::new( 1.0,  1.0, 1.0),
            Vec3::new(-1.0,  1.0, 1.0),
            // line [12]
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new( 1.0, -1.0, 0.0),
        ];

        for point_cs in clip_space_points {
            let point_ws = clip_to_world.project_point3(point_cs);
    
            res.push(point_ws);
        }

        res
    }

    fn get_camera_frustum_aabb_impl(clip_to_world: Mat4) -> AABB {
        let mut res = AABB::new();

        let clip_space_points: [Vec3; 8] = [
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0,  1.0, 1.0),
            Vec3::new( 1.0, -1.0, 1.0),
            Vec3::new( 1.0,  1.0, 1.0),
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(-1.0,  1.0, 0.0),
            Vec3::new( 1.0, -1.0, 0.0),
            Vec3::new( 1.0,  1.0, 0.0)
        ];

        for point_cs in clip_space_points {
            let point_ws = clip_to_world.project_point3(point_cs);
    
            res.merge_point_vec3(point_ws);
        }

        res
    }
}

#[derive(Default)]
pub struct CameraBuilder {
    body: CameraBody,
    lens: CameraLens,
}

impl CameraBuilder {
    fn new() -> Self {
        Self {
            body: Default::default(),
            lens: Default::default(),
        }
    }

    pub fn position(mut self, pos: Vec3) -> Self {
        self.body.position = pos;
        self
    }

    pub fn rotation(mut self, rot: Quat) -> Self {
        self.body.rotation = rot;
        self
    }

    pub fn aspect_ratio(mut self, ar: f32) -> Self {
        self.lens.aspect_ratio = ar;
        self
    }

    pub fn fov_vertical_degrees(mut self, fov_vertical_degrees: f32) -> Self {
        self.lens.fov_vertical_degrees = fov_vertical_degrees;
        self
    }

    pub fn near_plane(mut self, near: f32) -> Self {
        self.lens.near_plane = near;
        self
    }

    pub fn build(self) -> Camera {
        let mut cam = Camera::default();
        cam.with_body(self.body);
        cam.with_lens(self.lens);
        cam
    }
}

pub trait CameraControl: Any {
    fn update(&self, prev_trans: CameraTransform) -> CameraTransform;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct CameraController {
    controls: Vec<Box<dyn CameraControl>>,
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct CameraFrameConstants {
    pub world_to_view: Mat4,
    pub view_to_world: Mat4,
    pub view_to_clip: Mat4,
    pub clip_to_view: Mat4,
}

impl CameraController {
    pub fn builder() -> CameraControllerBuilder {
        CameraControllerBuilder::new()
    }

    pub fn get_control_mut<T: CameraControl>(&mut self) -> &mut T {
        self.controls.iter_mut()
            .find_map(|control| { control.as_any_mut().downcast_mut::<T>() })
            .unwrap_or_else(|| panic!("No camera control {} is found", std::any::type_name::<T>()))
    }

    pub fn update(&mut self, camera: &mut Camera) {
        let mut prev_transform = camera.body.into();

        for control in self.controls.iter_mut() {
            let new_transform = control.update(prev_transform);

            prev_transform = new_transform;
        }

        camera.body = prev_transform.into();
    }

    pub fn update_batch<'a>(&'a mut self, iter: impl Iterator<Item = &'a mut Camera>) {
        for cam in iter {
            self.update(cam);
        }
    }
}

pub struct CameraControllerBuilder {
    controller: CameraController
}

impl CameraControllerBuilder {
    fn new() -> Self {
        Self {
            controller: CameraController {
                controls: Default::default(),
            }
        }
    }
    
    pub fn with(mut self, control: impl CameraControl) -> Self {
        self.controller.controls.push(Box::new(control));
        self
    }

    pub fn build(self) -> CameraController {
        self.controller
    }
}