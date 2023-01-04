use std::{sync::Arc, collections::HashMap};

use ash::vk;

use glam::{Affine3A, Vec2, Vec3, Quat};
use raven_core::{utility, asset::asset_registry::AssetHandle, render::camera::{Camera, controller::FirstPersonController, CameraFrameConstants}};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable, RenderGraphPassBinding};
use raven_rhi::{Rhi, backend::{ImageDesc, Image, AccessType}, global_bindless_descriptor};

use crate::{
    MeshRenderer, IblRenderer, SkyRenderer,
    MeshRasterScheme, MeshShadingContext,
    renderer::{
        mesh_renderer::{MeshHandle, MeshInstanceHandle},
        post_process_renderer::{PostProcessRenderer}, image_lut::ImageLut, lut_renderer::BrdfLutComputer, light_renderer::DirectionalLight,
    }, LightRenderer, DebugRenderer, auto_exposure::{AutoExposureAdjustment, ExposureState}
};
#[cfg(feature = "gpu_ray_tracing")]
use crate::renderer::gpu_path_tracing_renderer::GpuPathTracingRenderer;

pub enum RenderMode {
    Raster,
    GpuPathTracing,
}

pub struct WorldRenderer {
    // TODO: remove this, renderer only do render jobs
    main_camera: Option<(Camera, FirstPersonController)>,

    render_resolution: [u32; 2],

    sky_renderer: SkyRenderer,
    ibl_renderer: IblRenderer,

    mesh_renderer: MeshRenderer,
    light_renderer: LightRenderer,

    exposure_state: ExposureState,
    auto_exposure: AutoExposureAdjustment,
    post_process_renderer: PostProcessRenderer,

    debug_renderer: DebugRenderer,

    image_luts: Vec<ImageLut>,
    bindless_descriptor_set: vk::DescriptorSet, // global bindless resources descriptor

    #[cfg(feature = "gpu_ray_tracing")]
    gpu_ray_tracing_renderer: GpuPathTracingRenderer,
    #[cfg(feature = "gpu_ray_tracing")]
    need_reset_accum: bool,

    render_mode: RenderMode,
}

impl WorldRenderer {
    pub fn new(rhi: &Rhi, render_res: [u32; 2]) -> Self {
        let mut mesh_renderer = MeshRenderer::new(rhi, MeshRasterScheme::Deferred, render_res);

        let bindless_descriptor_set = global_bindless_descriptor::create_engine_global_bindless_descriptor_set(rhi);
        mesh_renderer.update_bindless_resource(bindless_descriptor_set);

        let mut image_luts = Vec::new();
        let brdf_lut = ImageLut::new(rhi, Box::new(BrdfLutComputer));
        
        let handle = mesh_renderer.add_bindless_image(brdf_lut.get_backing_image().clone());
        assert_eq!(handle.0, 0);

        image_luts.push(brdf_lut);

        let mut light_renderer = LightRenderer::new(rhi);
        let _light_handle = light_renderer.add_directional_light(DirectionalLight {
            direction: Quat::from_rotation_arc(Vec3::from((0.0, 0.0, -1.0)), Vec3::from((-0.32803, 0.90599, 0.26749))),
            color: Vec3::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            shadowed: true,
        });

        Self {
            main_camera: None,
            render_resolution: render_res,

            sky_renderer: SkyRenderer::new(),
            ibl_renderer: IblRenderer::new(rhi),

            mesh_renderer,
            light_renderer,

            auto_exposure: AutoExposureAdjustment::new(),
            exposure_state: Default::default(),
            post_process_renderer: PostProcessRenderer::new(rhi),

            debug_renderer: DebugRenderer::new(rhi),

            image_luts,
            bindless_descriptor_set,

            #[cfg(feature = "gpu_ray_tracing")]
            gpu_ray_tracing_renderer: GpuPathTracingRenderer::new(rhi),
            #[cfg(feature = "gpu_ray_tracing")]
            need_reset_accum: true,

            render_mode: RenderMode::Raster,
        }
    }

    // TODO: remove this, renderer only do render jobs
    pub fn update_camera(&mut self, mouse_delta: Vec2, is_left_mouse_holding: bool, input: &HashMap<&str, f32>) {
        if let Some((cam, controller)) = &mut self.main_camera {
            controller.update(
                cam, mouse_delta,
                is_left_mouse_holding,
                input["walk"], input["strafe"], input["lift"]
            );
        }
    }

    // TODO: remove this, renderer only do render jobs
    #[inline]
    pub fn get_camera_render_data(&self) -> CameraFrameConstants {
        if let Some((cam, _)) = &self.main_camera {
            cam.get_camera_render_data()
        } else {
            panic!("Main camera not set yet!");
        }
    }

    // TODO: remove this, renderer only do render jobs
    #[inline]
    pub fn get_camera_position(&self) -> Vec3 {
        if let Some((cam, _)) = &self.main_camera {
            cam.body.position
        } else {
            panic!("Main camera not set yet!");
        }
    }

    // TODO: remove this, renderer only do render jobs
    #[inline]
    pub fn get_camera_rotation(&self) -> Quat {
        if let Some((cam, _)) = &self.main_camera {
            cam.body.rotation
        } else {
            panic!("Main camera not set yet!");
        }
    }

    // TODO: remove this, renderer only do render jobs
    #[inline]
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.render_mode = mode;
    }

    #[inline]
    pub fn get_render_resolution(&self) -> [u32; 2] {
        self.render_resolution
    }

    pub fn add_cubemap_split(&mut self, rhi: &Rhi, asset_handles: &[Arc<AssetHandle>; 6]) {
        self.sky_renderer.add_cubemap_split(rhi, asset_handles);
    }

    pub fn add_mesh(&mut self, asset_handle: &Arc<AssetHandle>) -> MeshHandle {
        let handle = self.mesh_renderer.add_asset_mesh(asset_handle);

        #[cfg(feature = "gpu_ray_tracing")]
        self.gpu_ray_tracing_renderer.add_mesh(handle, &self.mesh_renderer);

        handle
    }

    #[inline]
    pub fn add_mesh_instance(&mut self, transform: Affine3A, handle: MeshHandle) -> MeshInstanceHandle {
        self.mesh_renderer.add_mesh_instance(transform, handle)
    }

    // TODO: move to scene
    #[inline]
    pub fn set_main_camera(&mut self, camera: Camera, controller: FirstPersonController) {
        self.main_camera = Some((camera, controller));
    }

    pub fn update_pre_exposure(&mut self, dt: f32) {
        self.auto_exposure.update_ev(-self.post_process_renderer.image_log2_luminance(), dt);
        
        let ev_mult = (0.0 + self.auto_exposure.get_ev_smoothed()).exp2();

        self.exposure_state.pre_mult_prev_frame = self.exposure_state.pre_mult;

        // Smoothly blend the pre-exposure.
        // TODO: Ensure we correctly use the previous frame's pre-mult in temporal shaders,
        // and then nuke/speed-up this blending.
        self.exposure_state.pre_mult = self.exposure_state.pre_mult * 0.9 + ev_mult * 0.1;

        // Put the rest in post-exposure.
        self.exposure_state.post_mult = ev_mult / self.exposure_state.pre_mult;

        // update delta
        self.exposure_state.pre_mult_delta = self.exposure_state.pre_mult / self.exposure_state.pre_mult_prev_frame;
    }

    #[inline]
    pub fn current_exposure_state(&self) -> ExposureState {
        self.exposure_state
    }

    pub fn compute_image_lut_if_needed(&mut self, rg: &mut RenderGraphBuilder) {
        for image_lut in self.image_luts.iter_mut() {
            image_lut.compute_if_needed(rg);
        }
    }

    #[inline]
    #[cfg(feature = "gpu_ray_tracing")]
    pub fn reset_path_tracing_accumulation(&mut self) {
        self.need_reset_accum = true;
    }

    pub fn prepare_rg(&mut self, rg: &mut RenderGraphBuilder, dt: f32) -> RgHandle<Image> {
        self.update_pre_exposure(dt);
        self.compute_image_lut_if_needed(rg);

        let output = match self.render_mode {
            RenderMode::Raster => self.prepare_rg_raster(rg),
            RenderMode::GpuPathTracing => self.prepare_rg_gpu_path_tracing(rg),
        };

        output
    }

    fn prepare_rg_raster(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        let bindless_descriptor_set = self.bindless_descriptor_set;

        let main_img_desc = ImageDesc::new_2d(self.render_resolution, vk::Format::R32G32B32A32_SFLOAT);
        let mut main_img = rg.new_resource(main_img_desc);

        let cubemap = self.sky_renderer.get_cubemap();
        
        let is_cubemap_exist = cubemap.is_some();
        let cubemap_handle = if let Some(cubemap) = cubemap.as_ref() {
            Some(rg.import(cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
        } else {
            None
        };

        let (sh_buffer, prefilter_cubemap) = if is_cubemap_exist {
            let (sh, prefilter) = self.ibl_renderer.prepare_ibl_if_needed(rg, cubemap_handle.as_ref().unwrap());
            (Some(sh), Some(prefilter))
        } else {
            (None, None)
        };

        // shadow mapping
        let light_render_data = self.light_renderer.prepare_render_data(
            rg, &self.mesh_renderer
        );
        
        // mesh rasterization
        let (mut shading_context, light_maps) = self.mesh_renderer.prepare_rg(
            rg, light_render_data,
        );

        match &shading_context {
            // defer lighting
            MeshShadingContext::Defer(gbuffer) => {        
                let mut pass = rg.add_pass("gbuffer lighting");
                let pipeline = pass.register_compute_pipeline("defer/defer_lighting.hlsl");

                let gbuffer_img_ref = pass.read(&gbuffer.packed_gbuffer, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let depth_img_ref = pass.read(&gbuffer.depth, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let cubemap_ref = if let Some(cubemap) = cubemap_handle {
                    Some(pass.read(&cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                } else {
                    None
                };
                let sh_buffer_ref = if let Some(sh_buffer) = sh_buffer {
                    Some(pass.read(&sh_buffer, AccessType::AnyShaderReadUniformBuffer))
                } else {
                    None
                };
                let prefilter_cubemap_ref = if let Some(prefilter_cubemap) = prefilter_cubemap {
                    Some(pass.read(&prefilter_cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                } else {
                    None
                };
                let main_img_ref = pass.write(&mut main_img, AccessType::ComputeShaderWrite);
                
                let light_map_refs = light_maps.iter()
                    .map(|map| {
                        pass.read(&map, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer)
                    })
                    .collect::<Vec<_>>(); 

                let extent = gbuffer.packed_gbuffer.desc().extent;
                pass.render(move |ctx| {
                    let light_mat_offset = ctx.global_dynamic_buffer().previous_pushed_data_offset();

                    let mut depth_img_binding = depth_img_ref.bind();
                    depth_img_binding.with_aspect(vk::ImageAspectFlags::DEPTH);

                    let mut light_map_binding = light_map_refs.bind();
                    light_map_binding.with_aspect(vk::ImageAspectFlags::DEPTH);

                    // bind pipeline and descriptor set
                    let bound_pipeline = if is_cubemap_exist {
                        ctx.bind_compute_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[
                                gbuffer_img_ref.bind(),
                                depth_img_binding,
                                main_img_ref.bind(),
                                light_map_binding,
                                RenderGraphPassBinding::DynamicStorageBuffer(light_mat_offset),
                                cubemap_ref.unwrap().bind(),
                                sh_buffer_ref.unwrap().bind(),
                                prefilter_cubemap_ref.unwrap().bind(),
                            ])
                            .raw_descriptor_set(1, bindless_descriptor_set)
                        )?
                    } else {
                        ctx.bind_compute_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[
                                gbuffer_img_ref.bind(),
                                depth_img_binding,
                                main_img_ref.bind(),
                                light_map_binding,
                                RenderGraphPassBinding::DynamicStorageBuffer(light_mat_offset),
                            ])
                            .raw_descriptor_set(1, bindless_descriptor_set)
                        )?
                    };

                    let push_constants = [extent[0], extent[1]];
                    bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, utility::as_byte_slice_val(&push_constants));
                    
                    bound_pipeline.dispatch(extent);

                    Ok(())
                });
            },
            _ => unimplemented!(),
        }
        
        let mut post_img = self.post_process_renderer.prepare_rg(
            rg, main_img,
            self.exposure_state.post_mult, 1.0
        );

        // match (&self.frame_count, &self.main_camera) {
            //     (0, Some(main_cam)) => {
                //         self.debug_renderer.add_debug_line_lists(main_cam.0.get_camera_frustum_line_lists());
                
                //         self.cam_aabb = main_cam.0.get_camera_frustum_aabb();
                //         self.debug_renderer.add_debug_aabb(self.cam_aabb);
                //     }
                
                //     (_, Some(_)) => {
                    //         self.debug_renderer.add_debug_aabb(self.cam_aabb);
                    //     }
                    
        //     _ => {}
        // }
        
        self.debug_renderer.add_debug_aabb(self.mesh_renderer.get_scene_aabb());
        match &mut shading_context {
            MeshShadingContext::Defer(gbuffer) => {
                self.debug_renderer.prepare_rg(rg, &mut post_img, &mut gbuffer.depth);
            }
            _ => unimplemented!(),
        }
        self.debug_renderer.remove_all_aabbs();

        post_img
    }

    #[cfg(feature = "gpu_ray_tracing")]
    fn prepare_rg_gpu_path_tracing(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        use raven_rg::{GetOrCreateTemporal, image_clear};

        let mut accum_img = rg.get_or_create_temporal(
            "path tracing accum image",
            ImageDesc::new_2d(self.render_resolution, vk::Format::R32G32B32A32_SFLOAT)
                .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_DST)
        ).expect("Failed to create path tracing accumulation image!");

        if self.need_reset_accum {
            image_clear::clear_color(rg, &mut accum_img, [0.0, 0.0, 0.0, 0.0]);
            self.need_reset_accum = false;
        }

        let cubemap = rg.import(
            self.sky_renderer.get_cubemap().as_ref().unwrap().clone(),
            AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer
        );

        let tlas = self.gpu_ray_tracing_renderer.update_tlas(rg, &self.mesh_renderer);
        self.gpu_ray_tracing_renderer.path_tracing_accum(rg, &tlas, &mut accum_img, &cubemap, self.bindless_descriptor_set);

        let post_img = self.post_process_renderer.prepare_rg(
            rg, accum_img,
            self.exposure_state.post_mult, 1.0
        );

        post_img
    }

    #[cfg(not(feature = "gpu_ray_tracing"))]
    fn prepare_rg_gpu_path_tracing(&mut self, _rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        panic!("gpu ray tracing is not support!");
    }

    pub fn clean(self, rhi: &Rhi) {
        drop(self.image_luts);

        self.debug_renderer.clean();
        self.post_process_renderer.clean(rhi);

        self.ibl_renderer.clean(rhi);
        self.sky_renderer.clean(rhi);

        #[cfg(feature = "gpu_ray_tracing")]
        self.gpu_ray_tracing_renderer.clean(rhi);

        self.light_renderer.clean();
        self.mesh_renderer.clean(rhi);
    }
}