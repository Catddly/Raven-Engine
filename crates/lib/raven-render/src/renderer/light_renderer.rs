use std::sync::Arc;

use ash::vk;
use glam::{Vec3, Mat4, Quat};

use raven_core::{math::AABB};
use raven_rg::{RgHandle, RenderGraphBuilder};
use raven_rhi::{
    Rhi,
    backend::{
        Device, ImageDesc, Image, AccessType
    }
};

use crate::MeshRenderer;

const MAX_DIRECTIONAL_LIGHT_COUNT: usize = 10;

// TODO: move this to scene
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub direction: Quat,
    pub color: Vec3,
    // TODO: use lux as the light intensity parameter exposed to user
    pub intensity: f32,
    pub shadowed: bool,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct LightHandle(u32);

pub struct LightRenderData {
    pub light_matrices: Vec<Mat4>,
    pub light_maps: Vec<RgHandle<Image>>,
}

pub struct LightRenderer {
    directional_lights: Vec<DirectionalLight>,
    directional_light_maps: Vec<(u32, Arc<Image>)>,

    next_light_index: u32,

    device: Arc<Device>,
}

pub(crate) const SHADOW_MAP_DEFAULT_RESOLUTION: u32 = 2048;
pub(crate) const SHADOW_MAP_DEFAULT_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

impl LightRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        Self {
            directional_lights: Default::default(),
            directional_light_maps: Default::default(),

            next_light_index: 0,
            device: rhi.device.clone(),
        }
    }

    pub fn add_directional_light(&mut self, light: DirectionalLight) -> LightHandle {
        let next_directional_light_index = self.directional_lights.len();
        if next_directional_light_index < MAX_DIRECTIONAL_LIGHT_COUNT {
            let light_handle = LightHandle(self.next_light_index);

            if light.shadowed {
                let shadow_map = self.device.create_image(
                ImageDesc::new_2d(
                    [SHADOW_MAP_DEFAULT_RESOLUTION, SHADOW_MAP_DEFAULT_RESOLUTION],
                        SHADOW_MAP_DEFAULT_FORMAT,
                    )
                    .usage_flags(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST),
                    None
                )
                .expect("Failed to create shadow map for directional lights!");
                
                self.directional_light_maps.push((next_directional_light_index as u32, Arc::new(shadow_map)));
            }

            self.directional_lights.push(light);

            self.next_light_index += 1;

            return light_handle;
        } else {
            glog::warn!("At most 10 directional lights! Exceed boundary!");
            return LightHandle(u32::MAX)
        }
    }

    pub fn prepare_render_data(
        &self,
        rg: &mut RenderGraphBuilder,
        mesh_renderer: &MeshRenderer,
    ) -> LightRenderData {
        let scene_aabb = mesh_renderer.get_scene_aabb();
        let light_matrices = self.calculate_directional_light_matrix(scene_aabb);

        let light_maps = self.directional_light_maps.iter()
            .map(|(_, map)| {
                rg.import(map.clone(), AccessType::Nothing)
            })
            .collect::<Vec<_>>();

        assert_eq!(light_matrices.len(), light_maps.len());

        LightRenderData { 
            light_matrices,
            light_maps,
        }
    }

    fn calculate_directional_light_matrix(&self, scene_aabb: AABB) -> Vec<Mat4> {
        // TODO: remove render camera frustum culling for now
        let mut light_matrices = Vec::with_capacity(self.directional_light_maps.len());

        for (idx, _) in &self.directional_light_maps {
            let idx = *idx as usize;
            let light = &self.directional_lights[idx];
            // let frustum_aabb_center = camera_frustum_aabb.get_center();
            // let frustum_aabb_extent = camera_frustum_aabb.get_extent();

            let light_direction = light.direction.mul_vec3(Vec3::from((0.0, 0.0, -1.0))).normalize();

            let eye = scene_aabb.get_center() + light_direction * scene_aabb.get_extent();
            let world_to_view = Mat4::look_at_rh(eye, scene_aabb.get_center(), Vec3::new(0.0, 1.0, 0.0));

            let mut scene_aabb_vs = scene_aabb.clone();
            scene_aabb_vs.transform(world_to_view);

            // let mut frustum_aabb_vs = camera_frustum_aabb.clone();
            // frustum_aabb_vs.transform(world_to_view);

            // let view_to_clip = Mat4::orthographic_rh(
            //     f32::max(scene_aabb_vs.min.x, frustum_aabb_vs.min.x),
            //     f32::min(scene_aabb_vs.max.x, frustum_aabb_vs.max.x),
            //     f32::max(scene_aabb_vs.min.y, frustum_aabb_vs.min.y),
            //     f32::min(scene_aabb_vs.max.y, frustum_aabb_vs.max.y),
            //     // we use reverse z (far is near, near is far)
            //     -f32::max(scene_aabb_vs.min.z, frustum_aabb_vs.min.z),
            //     // there may be geometry really close to the camera
            //     -scene_aabb_vs.max.z,
            // );

            let view_to_clip = Mat4::orthographic_rh(
                scene_aabb_vs.min.x,
                scene_aabb_vs.max.x,
                scene_aabb_vs.min.y,
                scene_aabb_vs.max.y,
                // we use reverse z (far is near, near is far)
                -scene_aabb_vs.min.z,
                -scene_aabb_vs.max.z,
            );

            light_matrices.push(view_to_clip * world_to_view);
        }

        light_matrices
    }

    pub fn clean(self) {
        for (_, shadow_map) in self.directional_light_maps {
            let shadow_map = Arc::try_unwrap(shadow_map)
                .expect("Directional shadow map reference counts may not be retained!");

            self.device.destroy_image(shadow_map);
        }
    }
}