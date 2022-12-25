use std::sync::Arc;

use ash::vk;

use glam::Affine3A;
use raven_core::{utility::as_byte_slice_values, asset::asset_registry::AssetHandle};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable, GetOrCreateTemporal, image_clear};
use raven_rhi::{Rhi, backend::{ImageDesc, Image, AccessType}};

use crate::{
    MeshRenderer, IblRenderer, SkyRenderer,
    MeshRasterScheme, MeshShadingContext,
    renderer::{
        mesh_renderer::{MeshHandle, MeshInstanceHandle},
        post_process_renderer::{PostProcessRenderer, self}, image_lut::ImageLut, lut_renderer::BrdfLutComputer,
    }
};
#[cfg(feature = "gpu_ray_tracing")]
use crate::renderer::gpu_path_tracing_renderer::GpuPathTracingRenderer;

pub struct AutoExposureAdjustment {
    pub speed_log2: f32,

    ev_fast: f32,
    ev_slow: f32,

    enabled: bool,
}

pub enum RenderMode {
    Raster,
    GpuPathTracing,
}

impl AutoExposureAdjustment {
    pub fn new() -> Self {
        Self {
            speed_log2: 2.5_f32.log2(),

            ev_fast: 0.0,
            ev_slow: 0.0,

            // TODO: change to runtime variable
            enabled: post_process_renderer::ENABLE_AUTO_EXPOSURE,
        }
    }

    /// Get the smoothed transitioned exposure value
    pub fn get_ev_smoothed(&self) -> f32 {
        const DYNAMIC_EXPOSURE_BIAS: f32 = -2.0;

        if self.enabled {
            (self.ev_slow + self.ev_fast) * 0.5 + DYNAMIC_EXPOSURE_BIAS
        } else {
            0.0
        }
    }

    pub fn update_ev(&mut self, ev: f32, dt: f32) {
        if !self.enabled {
            return;
        }

        let ev = ev.clamp(post_process_renderer::LUMINANCE_HISTOGRAM_MIN_LOG2 as f32, post_process_renderer::LUMINANCE_HISTOGRAM_MAX_LOG2 as f32);

        let dt = dt * self.speed_log2.exp2(); // reverse operation

        let t_fast = 1.0 - (-1.0 * dt).exp();
        self.ev_fast = (ev - self.ev_fast) * t_fast + self.ev_fast;

        let t_slow = 1.0 - (-0.25 * dt).exp();
        self.ev_slow = (ev - self.ev_slow) * t_slow + self.ev_slow;
    }
}

#[derive(Clone, Copy)]
pub struct ExposureState {
    pub pre_mult: f32,
    pub post_mult: f32,

    pub pre_mult_prev_frame: f32,
    // pre_mult / pre_mult_prev_frame
    pub pre_mult_delta: f32,
}

impl Default for ExposureState {
    fn default() -> Self {
        Self {
            pre_mult: 1.0,
            post_mult: 1.0,
            pre_mult_prev_frame: 1.0,
            pre_mult_delta: 1.0,
        }
    }
}

pub struct WorldRenderer {
    render_resolution: [u32; 2],

    sky_renderer: SkyRenderer,
    ibl_renderer: IblRenderer,

    mesh_renderer: MeshRenderer,

    auto_exposure: AutoExposureAdjustment,
    exposure_state: ExposureState,
    post_process_renderer: PostProcessRenderer,

    #[cfg(feature = "gpu_ray_tracing")]
    gpu_ray_tracing_renderer: GpuPathTracingRenderer,
    #[cfg(feature = "gpu_ray_tracing")]
    need_reset_accum: bool,

    render_mode: RenderMode,
}

impl WorldRenderer {
    pub fn new(rhi: &Rhi, render_res: [u32; 2]) -> Self {
        let brdf_lut = ImageLut::new(rhi, Box::new(BrdfLutComputer));
        
        let mut mesh_renderer = MeshRenderer::new(rhi, MeshRasterScheme::Deferred, render_res);
        let handle = mesh_renderer.add_bindless_image_lut(brdf_lut);
        assert_eq!(handle.0, 0);

        Self {
            render_resolution: render_res,

            sky_renderer: SkyRenderer::new(),
            ibl_renderer: IblRenderer::new(rhi),
            mesh_renderer,

            auto_exposure: AutoExposureAdjustment::new(),
            exposure_state: Default::default(),
            post_process_renderer: PostProcessRenderer::new(rhi),

            #[cfg(feature = "gpu_ray_tracing")]
            gpu_ray_tracing_renderer: GpuPathTracingRenderer::new(rhi),
            #[cfg(feature = "gpu_ray_tracing")]
            need_reset_accum: true,

            render_mode: RenderMode::Raster,
        }
    }

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

    pub fn add_mesh_instance(&mut self, transform: Affine3A, handle: MeshHandle) -> MeshInstanceHandle {
        self.mesh_renderer.add_mesh_instance(transform, handle)
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

    pub fn current_exposure_state(&self) -> ExposureState {
        self.exposure_state
    }

    pub fn prepare_rg(&mut self, rg: &mut RenderGraphBuilder, dt: f32) -> RgHandle<Image> {
        self.update_pre_exposure(dt);

        self.mesh_renderer.compute_lut_if_needed(rg);

        match self.render_mode {
            RenderMode::Raster => self.prepare_rg_raster(rg),
            RenderMode::GpuPathTracing => self.prepare_rg_gpu_path_tracing(rg),
        }
    }

    fn prepare_rg_raster(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        let main_img_desc: ImageDesc = ImageDesc::new_2d(self.render_resolution, vk::Format::R32G32B32A32_SFLOAT);
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
        
        // mesh rasterization
        let shading_context = self.mesh_renderer.prepare_rg(rg);

        // lighting
        match &shading_context {
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

                let bindless_descriptor_set = self.mesh_renderer.bindless_descriptor_set();

                let extent = gbuffer.packed_gbuffer.desc().extent;
                pass.render(move |context| {
                    let mut depth_img_binding = depth_img_ref.bind();
                    depth_img_binding.with_aspect(vk::ImageAspectFlags::DEPTH);

                    // bind pipeline and descriptor set
                    let bound_pipeline = if is_cubemap_exist {
                        context.bind_compute_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[
                                gbuffer_img_ref.bind(),
                                depth_img_binding,
                                main_img_ref.bind(),
                                cubemap_ref.unwrap().bind(),
                                sh_buffer_ref.unwrap().bind(),
                                prefilter_cubemap_ref.unwrap().bind(),
                            ])
                            .raw_descriptor_set(1, bindless_descriptor_set)
                        )?
                    } else {
                        context.bind_compute_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[
                                gbuffer_img_ref.bind(),
                                depth_img_binding,
                                main_img_ref.bind()
                            ])
                        )?
                    };

                    let push_constants = [extent[0], extent[1]];
                    bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
                    
                    bound_pipeline.dispatch(extent);

                    Ok(())
                });
            },
            _ => unimplemented!(),
        }
        
        let post_img = self.post_process_renderer.prepare_rg(
            rg, main_img,
            self.exposure_state.post_mult, 1.0
        );

        post_img
    }

    pub fn reset_path_tracing_accumulation(&mut self) {
        self.need_reset_accum = true;
    }

    #[cfg(feature = "gpu_ray_tracing")]
    fn prepare_rg_gpu_path_tracing(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        let mut accum_img = rg.get_or_create_temporal(
            "path tracing accum image",
            ImageDesc::new_2d(self.render_resolution, vk::Format::R32G32B32A32_SFLOAT)
                .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_DST)
        ).expect("Failed to create path tracing accumulation image!");

        if self.need_reset_accum {
            image_clear::clear_color(rg, &mut accum_img, [0.0, 0.0, 0.0, 0.0]);
            self.need_reset_accum = false;
        }

        let bindless_descriptor_set = self.mesh_renderer.bindless_descriptor_set();

        let cubemap = rg.import(
            self.sky_renderer.get_cubemap().as_ref().unwrap().clone(),
            AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer
        );

        let tlas = self.gpu_ray_tracing_renderer.update_tlas(rg, &self.mesh_renderer);
        self.gpu_ray_tracing_renderer.path_tracing_accum(rg, &tlas, &mut accum_img, &cubemap, bindless_descriptor_set);

        let post_img = self.post_process_renderer.prepare_rg(
            rg, accum_img,
            self.exposure_state.post_mult, 1.0
        );

        post_img
    }

    #[cfg(not(feature = "gpu_ray_tracing"))]
    fn prepare_rg_gpu_path_tracing(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        panic!("gpu ray tracing is not support!");
    }

    pub fn clean(self, rhi: &Rhi) {
        self.post_process_renderer.clean(rhi);

        self.ibl_renderer.clean(rhi);
        self.sky_renderer.clean(rhi);

        #[cfg(feature = "gpu_ray_tracing")]
        self.gpu_ray_tracing_renderer.clean(rhi);
        self.mesh_renderer.clean(rhi);
    }
}