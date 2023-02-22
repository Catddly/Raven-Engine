use std::{ops::Deref};
use std::sync::{Arc};
use std::collections::HashMap;

use parking_lot::RwLock;

use raven_asset::asset_registry::AssetHandle;
pub use raven_rhi::{RhiConfig};
pub use raven_rg::{RgHandle, LightFrameConstants, FrameConstants};
pub use raven_render::{*};

use raven_rhi::{Rhi, backend::AccessType};
use raven_rg::{GraphExecutor, IntoPipelineDescriptorBindings, RenderGraphPassBindable};
use raven_math::{Vec2, Vec3, Quat, Affine3A};
use raven_scene::camera::{CameraFrameConstants, Camera, controller::FirstPersonController};

type PrepareFrameResult = anyhow::Result<()>;

#[non_exhaustive]
pub struct RenderApiInner {
    rhi: Rhi,
    rg_executor: GraphExecutor,
    renderer: WorldRenderer,

    prepare_frame_result: Option<PrepareFrameResult>,
    frame_index: u32,
}

impl std::fmt::Debug for RenderApiInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug Default RenderApiImpl")
    }
}

impl RenderApiInner {
    fn new() -> Self {
        let core_api = crate::core::get();
        
        let read_guard = core_api.read();
        let main_window = read_guard.main_window();
        let render_resolution = main_window.inner_size();
        let render_resolution = [render_resolution.width, render_resolution.height];

        let rhi_config = RhiConfig {
            enable_debug: true,
            enable_vsync: false,
            swapchain_extent: main_window.inner_size().into(),
        };

        let rhi = Rhi::new(rhi_config, main_window)
            .expect("Failed to create render rhi (vulkan)!");
        let rg_executor = GraphExecutor::new(&rhi)
            .expect("Failed to create render graph!");
        let renderer = WorldRenderer::new(&rhi, render_resolution);

        Self {
            rhi,
            rg_executor,
            renderer,

            prepare_frame_result: None,
            frame_index: 0,
        }
    }

    #[inline]
    pub fn add_cubemap_split(&mut self, asset_handles: &[Arc<AssetHandle>; 6]) {
        self.renderer.add_cubemap_split(&self.rhi, asset_handles)
    }

    #[inline]
    pub fn add_mesh(&mut self, asset_handle: &Arc<AssetHandle>) -> MeshHandle {
        self.renderer.add_mesh(asset_handle)
    }

    #[inline]
    pub fn add_mesh_instance(&mut self, handle: MeshHandle, transform: Affine3A) -> MeshInstanceHandle {
        self.renderer.add_mesh_instance(handle, transform)
    }

    #[inline]
    pub fn get_render_resolution(&self) -> [u32; 2] {
        self.renderer.get_render_resolution()
    }

    #[inline]
    pub fn set_main_camera(&mut self, camera: Camera, controller: FirstPersonController) {
        self.renderer.set_main_camera(camera, controller)
    }

    #[inline]
    pub fn update_camera(&mut self, mouse_delta: Vec2, is_left_mouse_holding: bool, input: &HashMap<&str, f32>) {
        self.renderer.update_camera(mouse_delta, is_left_mouse_holding, input)
    }

    #[inline]
    pub fn get_camera_render_data(&self) -> CameraFrameConstants{
        self.renderer.get_camera_render_data()
    }

    #[inline]
    pub fn get_camera_position(&self) -> Vec3 {
        self.renderer.get_camera_position()
    }

    #[inline]
    pub fn get_camera_rotation(&self) -> Quat {
        self.renderer.get_camera_rotation()
    }

    #[inline]
    #[cfg(feature = "gpu_ray_tracing")]
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.renderer.set_render_mode(mode)
    }

    #[inline]
    #[cfg(feature = "gpu_ray_tracing")]
    pub fn reset_path_tracing_accumulation(&mut self) {
        self.renderer.reset_path_tracing_accumulation()
    }

    pub fn prepare_frame(&mut self, dt: f32) {
        let resolution = self.renderer.get_render_resolution();

        let prepare_result = self.rg_executor.prepare(|rg| {
            let main_img = self.renderer.prepare_rg(rg, dt);

            // copy final image to swapchain
            let mut swapchain_img = rg.get_swapchain(resolution);
            
            let mut pass = rg.add_pass("final blit");
            let pipeline = pass.register_compute_pipeline("image_blit.hlsl");
                            
            let main_img_ref = pass.read(&main_img, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let swapchain_img_ref = pass.write(&mut swapchain_img, AccessType::ComputeShaderWrite);

            pass.render(move |ctx| {
                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        main_img_ref.bind(), 
                        swapchain_img_ref.bind()
                    ])
                )?;

                bound_pipeline.dispatch([resolution[0], resolution[1], 1]);

                Ok(())
            });
        });

        self.prepare_frame_result = Some(prepare_result);
    }

    pub fn draw_frame(&mut self, mut frame_constants: FrameConstants) {
        let exposure_state = self.renderer.current_exposure_state();

        frame_constants.pre_exposure_mult = exposure_state.pre_mult;
        frame_constants.pre_exposure_prev_frame_mult = exposure_state.pre_mult_prev_frame;
        frame_constants.pre_exposure_delta = exposure_state.pre_mult_delta;

        let prepare_result = self.prepare_frame_result.take()
            .expect("Require current frame to be prepared to do drawing!");

        match prepare_result {
            Ok(()) => {
                self.rg_executor.draw(
                    &frame_constants,
                    &mut self.rhi.swapchain
                );

                self.frame_index = self.frame_index.wrapping_add(1);
            },
            Err(err) => {
                panic!("Failed to prepare render graph with {:?}", err);
            }
        }
    }

    #[inline]
    pub fn get_frame_prepare_result(&self) -> &PrepareFrameResult {
        self.prepare_frame_result.as_ref()
           .expect("Please call prepare_frame() first!")
    }

    #[inline]
    pub fn device_wait_idle(&self) {
        self.rhi.device.wait_idle()
    }

    #[inline]
    pub fn current_frame_index(&self) -> u32 {
        self.frame_index
    }
}

#[derive(Clone)]
pub struct RenderApiImpl(Option<Arc<RwLock<RenderApiInner>>>);

unsafe impl Send for RenderApiImpl {}
unsafe impl Sync for RenderApiImpl {}

impl Deref for RenderApiImpl {
    type Target = Arc<RwLock<RenderApiInner>>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl RenderApiImpl {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn init(&mut self) {
        self.0 = Some(Arc::new(RwLock::new(RenderApiInner::new())));

    }

    pub fn shutdown(mut self) {
        if let Some(inner) = self.0.take() {
            let inner = Arc::try_unwrap(inner)
                .expect("Reference counting of render api may not be retained!");
            let inner = inner.into_inner();
            
            inner.device_wait_idle();

            inner.renderer.clean(&inner.rhi);
            inner.rg_executor.shutdown();
        } else {
            panic!("Try to shutdown render apis before initializing!");
        }
    }
}