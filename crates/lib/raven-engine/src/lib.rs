mod user;
pub mod prelude;

use std::{collections::VecDeque};

use ash::vk;
use glam::{Vec3, Quat};
use turbosloth::*;

use raven_core::{
    winit::{
        event_loop::{EventLoop},
        dpi::{LogicalSize, LogicalPosition},
        window::{Window, WindowBuilder},
        event::{WindowEvent, Event, VirtualKeyCode, ElementState}, 
        platform::run_return::EventLoopExtRunReturn
    }, asset::{loader::{mesh_loader::GltfMeshLoader, AssetLoader}, AssetType, AssetProcessor}, concurrent::executor, render::camera::{self}, input::InputBinding, utility::as_byte_slice_values,
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::input::{InputManager, KeyCode, MouseButton};
use raven_core::filesystem::{self, ProjectFolder};
use raven_render::{MeshRenderer, MeshRasterScheme, MeshShadingContext};
use raven_rhi::{RHIConfig, Rhi, backend};
use raven_rg::{Executor, IntoPipelineDescriptorBindings, RenderGraphPassBindable, DrawFrameContext};

use raven_core::system::OnceQueue;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
pub struct EngineContext<App> {
    pub main_window: Window,
    event_loop: EventLoop<()>,
    input_manager: InputManager,

    rhi: Rhi,
    rg_executor: Executor,

    app: App,
}

fn init_filesystem_module() -> anyhow::Result<()> {
    filesystem::set_default_root_path()?;
    
    filesystem::set_custom_mount_point(ProjectFolder::Baked, "../../resource/baked/")?;
    filesystem::set_custom_mount_point(ProjectFolder::Assets, "../../resource/assets/")?;
    filesystem::set_custom_mount_point(ProjectFolder::ShaderSource, "../../resource/shader_src/")?;

    Ok(())
}

fn init_log_module() -> anyhow::Result<()> {
    let console_var = console::from_args();

    log::init_log(log::LogConfig {
        level: console_var.level,
    })?;

    Ok(())
}

/// Initialize raven engine.
pub fn init(app: impl user::App) -> anyhow::Result<EngineContext<impl user::App>> {
    let mut init_queue = OnceQueue::new();

    init_queue.push_job(init_filesystem_module);
    init_queue.push_job(init_log_module);

    init_queue.execute()?;

    // init event loop
    let event_loop = EventLoop::new();
    let primary_monitor = event_loop.primary_monitor()
        .expect("Must have at least one monitor!");
    primary_monitor.video_modes()
        .next()
        .expect("Must have at least one video modes!");

    let scale_factor = primary_monitor.scale_factor();
    let monitor_resolution = primary_monitor.size().to_logical::<f64>(scale_factor);

    let window_resolution = LogicalSize::new(
        1920.0,
        1080.0
    );
    let window_position = LogicalPosition::new (
        (monitor_resolution.width - window_resolution.width) / 2.0,
        (monitor_resolution.height - window_resolution.height) / 2.0,
    );  

    // create main window
    let main_window = WindowBuilder::new()
        .with_inner_size(window_resolution)
        .with_position(window_position)
        .with_resizable(false)
        .with_title("Raven Engine")
        .build(&event_loop)
        .expect("Failed to create a window!");

    // create render device
    let rhi_config = RHIConfig {
        enable_debug: true,
        enable_vsync: false,
        swapchain_extent: main_window.inner_size().into(),
    };
    let rhi = Rhi::new(rhi_config, &main_window)?;

    let rg_executor = Executor::new(&rhi)?;

    let mut app = app;
    app.init()?;

    glog::trace!("Raven Engine initialized!");
    Ok(EngineContext { 
        main_window,
        event_loop,
        input_manager: InputManager::new(),

        rhi,
        rg_executor,

        app,
    })
}

/// Start engine main loop.
pub fn main_loop(engine_context: &mut EngineContext<impl user::App>) {
    let EngineContext { 
        event_loop,
        main_window,
        input_manager,
        
        rhi,
        rg_executor,

        app,
    } = engine_context;

    let render_resolution = main_window.inner_size();
    let render_resolution = [render_resolution.width, render_resolution.height];

    // temporary
    let main_img_desc: backend::ImageDesc = backend::ImageDesc::new_2d(render_resolution, vk::Format::R8G8B8A8_UNORM);

    let loader = Box::new(GltfMeshLoader::new(std::path::PathBuf::from("mesh/roughness_scale/scene.gltf"))) as Box<dyn AssetLoader>;
    let raw_asset = loader.load().unwrap();

    assert!(matches!(raw_asset.asset_type(), AssetType::Mesh));

    let processor = AssetProcessor::new("mesh/roughness_scale/scene.gltf", raw_asset);
    let handle = processor.process().unwrap();
    let lazy_cache = LazyCache::create();

    let task = executor::spawn(handle.eval(&lazy_cache));
    let handle = smol::block_on(task).unwrap();

    let mut mesh_renderer = MeshRenderer::new(rhi, MeshRasterScheme::Deferred, render_resolution);
    mesh_renderer.add_asset_mesh(&handle);

    let mut camera = camera::Camera::builder()
        .aspect_ratio(render_resolution[0] as f32 / render_resolution[1] as f32)
        .build();
    let mut camera_controller = camera::controller::FirstPersonController::new(Vec3::new(0.0, 1.0, 3.0), Quat::IDENTITY);

    let mut static_events = Vec::new();
    let mut last_frame_time = std::time::Instant::now();

    const FILTER_FRAME_COUNT: usize = 10;
    let mut dt_filter_queue = VecDeque::with_capacity(FILTER_FRAME_COUNT);

    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::W), 
        InputBinding::new("walk", 1.0).activation_time(0.2)
    );
    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::S), 
        InputBinding::new("walk", -1.0).activation_time(0.2)
    );
    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::A), 
        InputBinding::new("strafe", -1.0).activation_time(0.2)
    );
    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::D), 
        InputBinding::new("strafe", 1.0).activation_time(0.2)
    );
    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::Q), 
        InputBinding::new("lift", -1.0).activation_time(0.2)
    );
    input_manager.add_binding(
        KeyCode::vkcode(VirtualKeyCode::E), 
        InputBinding::new("lift", 1.0).activation_time(0.2)
    );

    let mut running = true;
    while running { // main loop start
        // filter delta time to get a smooth result for simulation and rendering
        let dt = {
            let now = std::time::Instant::now();
            let delta = now - last_frame_time;
            last_frame_time = now;

            let delta_desc = delta.as_secs_f32();

            if dt_filter_queue.len() >= FILTER_FRAME_COUNT {
                dt_filter_queue.pop_front();
            }
            dt_filter_queue.push_back(delta_desc);

            dt_filter_queue.iter().copied().sum::<f32>() / (dt_filter_queue.len() as f32)
        };

        // tick logic begin
        let draw_frame_context = {
            // system messages
            event_loop.run_return(|event, _, control_flow| {
                control_flow.set_poll();
        
                match &event {
                    Event::WindowEvent {
                        event,
                        ..
                    } => match event {
                        WindowEvent::KeyboardInput { 
                            input,
                            ..
                        } => {
                            if Some(VirtualKeyCode::Escape) == input.virtual_keycode && input.state == ElementState::Released {
                                control_flow.set_exit();
                                running = false;
                            }
                        }
                        WindowEvent::CloseRequested => {
                            control_flow.set_exit();
                            running = false;
                        }
                        WindowEvent::Resized(physical_size) => {
                            glog::trace!("Window resized (Physical): [{}, {}]", physical_size.width, physical_size.height);
                        }
                        _ => {}
                    },
                    Event::MainEventsCleared => {
                        control_flow.set_exit();
                    }
                    _ => (),
                }

                static_events.extend(event.to_static());
            });

            input_manager.update(&static_events);
            let input = input_manager.map(dt);
            let mouse_delta = input_manager.mouse_pos_delta() * dt;

            camera_controller.update(&mut camera, mouse_delta, input_manager.is_mouse_hold(MouseButton::LEFT),
                input["walk"], input["strafe"], input["lift"]);
            let cam_matrices = camera.camera_matrices();

            // user-side app tick
            app.tick(dt);

            static_events.clear();

            DrawFrameContext {
                cam_matrices,
            }
        };
        // tick render end
        
        // tick render begin
        {
            // prepare and compile render graph
            let prepare_result = rg_executor.prepare(|rg| {
                // No renderer yet!
                let mut main_img = rg.new_resource(main_img_desc);
                {
                    // mesh rasterize
                    let shading_context = mesh_renderer.prepare_rg(rg);
    
                    // lighting
                    match shading_context {
                        MeshShadingContext::GBuffer(gbuffer) => {        
                            let mut pass = rg.add_pass("gbuffer lighting");
                            let pipeline = pass.register_compute_pipeline("defer/defer_lighting.hlsl");
                                            
                            let gbuffer_img_ref = pass.read(&gbuffer.packed_gbuffer, backend::AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                            let depth_img_ref = pass.read(&gbuffer.depth, backend::AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                            //let geo_normal_img_ref = pass.read(&gbuffer.geometric_normal, backend::AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                            let main_img_ref = pass.write(&mut main_img, backend::AccessType::ComputeShaderWrite);
            
                            pass.render(move |context| {
                                let mut depth_img_binding = depth_img_ref.bind();
                                depth_img_binding.with_aspect(vk::ImageAspectFlags::DEPTH);
    
                                // bind pipeline and descriptor set
                                let bound_pipeline = context.bind_compute_pipeline(pipeline.into_bindings()
                                    .descriptor_set(0, &[
                                        gbuffer_img_ref.bind(),
                                        depth_img_binding,
                                        // geo_normal_img_ref.bind(),
                                        main_img_ref.bind()
                                    ])
                                )?;

                                let extent = gbuffer.packed_gbuffer.desc().extent;
                                let push_constants = [extent[0], extent[1]];
                                
                                bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
                                bound_pipeline.dispatch(extent);
            
                                Ok(())
                            });
                        },
                        _ => unimplemented!(),
                    }
                }
    
                // copy to swapchain
                let mut swapchain_img = rg.get_swapchain(render_resolution);
                {
                    let mut pass = rg.add_pass("final blit");
                    let pipeline = pass.register_compute_pipeline("image_blit.hlsl");
                                    
                    let main_img_ref = pass.read(&main_img, backend::AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                    let swapchain_img_ref = pass.write(&mut swapchain_img, backend::AccessType::ComputeShaderWrite);
    
                    pass.render(move |context| {
                        // bind pipeline and descriptor set
                        let bound_pipeline = context.bind_compute_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[
                                main_img_ref.bind(), 
                                swapchain_img_ref.bind()
                            ])
                        )?;
    
                        bound_pipeline.dispatch([render_resolution[0], render_resolution[1], 1]);
    
                        Ok(())
                    });
                }
            });
    
            // draw
            match prepare_result {
                Ok(()) => {
                    rg_executor.draw(
                        &draw_frame_context,
                        &mut rhi.swapchain
                    );
                },
                Err(err) => {
                    panic!("Failed to prepare render graph with {:?}", err);
                }
            }
        } // tick render end
    } // main loop end
    
    rhi.device.wait_idle();
    mesh_renderer.clean(rhi);
    glog::trace!("Exit main loop successfully!");
}

/// Shutdown raven engine.
pub fn shutdown(engine_context: EngineContext<impl user::App>) {
    engine_context.app.shutdown();
    engine_context.rg_executor.shutdown();
    glog::trace!("Raven Engine shutdown.");
}