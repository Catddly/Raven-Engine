use std::sync::Arc;

use ash::vk;
use glam::{Vec3, Mat4, Quat};

use raven_core::{math::AABB};
use raven_rg::{RgHandle, RenderGraphBuilder, IntoPipelineDescriptorBindings, RenderGraphPassBinding, image_clear};
use raven_rhi::{
    Rhi,
    backend::{
        Device, ImageDesc, Image, RasterPipelineDesc, PipelineShaderDesc, PipelineShaderStage,
        RenderPassDesc, renderpass, RenderPassAttachmentDesc, RenderPass, AccessType, ImageViewDesc, RasterPipelineCullMode
    }
};

use crate::MeshRenderer;

pub struct DirectionalLight {
    direction: Quat,
    // TODO: use lux as the light intensity parameter exposed to user
    _intensity: f32,
    _shadowed: bool,
}

pub struct LightRenderer {
    directional_lights: Vec<DirectionalLight>,
    directional_light_maps: Vec<Option<Arc<Image>>>,

    renderpass: Arc<RenderPass>,
    device: Arc<Device>,
}

const SHADOW_MAP_DEFAULT_RESOLUTION: u32 = 2048;
const SHADOW_MAP_DEFAULT_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

impl LightRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let renderpass = renderpass::create_render_pass(
            &rhi.device,
            RenderPassDesc {
                color_attachments: &[],
                depth_attachment: Some(
                    RenderPassAttachmentDesc::new(SHADOW_MAP_DEFAULT_FORMAT)
                )
            }
        );

        Self {
            directional_lights: Default::default(),
            directional_light_maps: Default::default(),

            renderpass,
            device: rhi.device.clone(),
        }
    }

    pub fn add_directional_light(&mut self, direction: Quat, _intensity: f32) {
        if self.directional_lights.is_empty() {
            self.directional_lights.push(DirectionalLight { 
                direction,
                _intensity,
                _shadowed: true
            });

            let shadow_map = self.device.create_image(
            ImageDesc::new_2d(
                [SHADOW_MAP_DEFAULT_RESOLUTION, SHADOW_MAP_DEFAULT_RESOLUTION],
                    SHADOW_MAP_DEFAULT_FORMAT,
                )
                .usage_flags(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST),
                None
            )
            .expect("Failed to create shadow map for directional lights!");
            
            self.directional_light_maps.push(Some(Arc::new(shadow_map)));
        }
    }

    pub fn prepare_rg(&mut self,
        rg: &mut RenderGraphBuilder,
        mesh_renderer: &MeshRenderer,
        bindless_descriptor_set: vk::DescriptorSet,
    ) -> (RgHandle<Image>, Vec<Mat4>) {
        let scene_aabb = mesh_renderer.get_scene_aabb();
        //let cam_frustum_aabb = cam.get_camera_frustum_aabb();
        let light_matrices = self.calculate_directional_light_matrix(scene_aabb);

        let mut directional_maps = Vec::with_capacity(self.directional_lights.len());

        // import all
        for i in 0..self.directional_lights.len() {
            if let Some(shadow_map) = self.directional_light_maps[i].clone() {
                let shadow_map = rg.import(shadow_map, AccessType::Nothing);
                directional_maps.push(shadow_map);
            }
        }

        let shadow_map = &mut directional_maps[0];
        image_clear::clear_depth_stencil(rg, shadow_map);

        let mut pass = rg.add_pass("shadow mapping");
        let pipeline = pass.register_raster_pipeline(&[
                PipelineShaderDesc::builder()
                    .stage(PipelineShaderStage::Vertex)
                    .source("shadow/shadow_mapping.hlsl")
                    .entry("vs_main")
                    .build().unwrap(),
                PipelineShaderDesc::builder()
                    .stage(PipelineShaderStage::Pixel)
                    .source("shadow/shadow_mapping.hlsl")
                    .entry("ps_main")
                    .build().unwrap(),
            ],
            RasterPipelineDesc::builder()
                .render_pass(self.renderpass.clone())
                .cull_mode(RasterPipelineCullMode::Front)
                .depth_bias(true)
                .build().unwrap()
        );

        // render directional lights
        {
            let renderpass = self.renderpass.clone();

            let light_matrix = light_matrices[0];
            let shadow_map_ref = pass.raster_write(shadow_map, AccessType::DepthAttachmentWriteStencilReadOnly);

            let draw_data_buffer = mesh_renderer.get_draw_data_buffer();
            // TODO: this would be copied every frame, any better idea?
            let meshes = mesh_renderer.get_uploaded_meshes().to_owned();
            let mesh_instances = mesh_renderer.get_mesh_instances().to_owned();

            pass.render(move |ctx| {
                let matrix_data = light_matrix.transpose().to_cols_array();
                let light_mat_offset = ctx.global_dynamic_buffer().push(&matrix_data);

                // TODO: this is a waste of memory, since we have push data once in the mesh renderer
                let xform_iter = mesh_instances.iter()
                    .map(|ins| {
                        // transpose to row-major matrix to be used in shader
                        let transform = [
                            ins.transform.x_axis.x,
                            ins.transform.y_axis.x,
                            ins.transform.z_axis.x,
                            ins.transform.translation.x,
                            ins.transform.x_axis.y,
                            ins.transform.y_axis.y,
                            ins.transform.z_axis.y,
                            ins.transform.translation.y,
                            ins.transform.x_axis.z,
                            ins.transform.y_axis.z,
                            ins.transform.z_axis.z,
                            ins.transform.translation.z,
                        ];

                        transform
                    });
                let instance_data_offset = ctx.global_dynamic_buffer().push_from_iter(xform_iter);

                ctx.begin_render_pass(
                    &renderpass,
                    [SHADOW_MAP_DEFAULT_RESOLUTION, SHADOW_MAP_DEFAULT_RESOLUTION],
                    &[],
                    Some((shadow_map_ref, &ImageViewDesc::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .build().unwrap())
                    )
                )?;
                ctx.set_default_viewport_and_scissor([SHADOW_MAP_DEFAULT_RESOLUTION, SHADOW_MAP_DEFAULT_RESOLUTION]);
                // Note: we use reverse-z, so the bias constant and slope factor here are all negative
                ctx.set_depth_bias(-0.1, 0.0, -0.25);

                let bound_pipeline = ctx.bind_raster_pipeline(
                    pipeline.into_bindings()
                        .descriptor_set(0, &[
                            RenderGraphPassBinding::DynamicStorageBuffer(light_mat_offset),
                            RenderGraphPassBinding::DynamicStorageBuffer(instance_data_offset),
                        ])
                        .raw_descriptor_set(1, bindless_descriptor_set)
                )?;

                MeshRenderer::mesh_draw_func(
                    &mesh_instances, &meshes, &draw_data_buffer, 
                    ctx, &bound_pipeline
                );

                ctx.end_render_pass();

                Ok(())
            });
        }

        (
            directional_maps.into_iter().next().expect("At least one directional light maps!"),
            light_matrices,
        )
    }

    fn calculate_directional_light_matrix(&self, scene_aabb: AABB) -> Vec<Mat4> {
        // TODO: remove render camera frustum culling for now

        let mut light_matrices = Vec::with_capacity(self.directional_lights.len());

        for light in &self.directional_lights {
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
        for shadow_map in self.directional_light_maps {
            if let Some(shadow_map) = shadow_map {
                let shadow_map = Arc::try_unwrap(shadow_map)
                    .expect("Directional shadow map reference counts may not be retained!");
    
                self.device.destroy_image(shadow_map);
            }
        }
    }
}