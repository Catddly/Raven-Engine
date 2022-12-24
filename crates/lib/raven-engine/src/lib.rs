mod user;
pub mod prelude;

use std::{collections::VecDeque, sync::Arc};

use glam::{Vec3, Quat, Affine3A};

use raven_core::{
    winit::{
        event_loop::{EventLoop},
        dpi::{LogicalSize, LogicalPosition},
        window::{Window, WindowBuilder},
        event::{WindowEvent, Event, VirtualKeyCode, ElementState}, 
        platform::run_return::EventLoopExtRunReturn
    }, asset::{AssetManager, AssetLoadDesc, asset_registry::AssetHandle}, 
    render::{camera::{self}, persistence::{PersistStates, IsStatesChanged}}, 
    input::InputBinding, 
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::input::{InputManager, KeyCode, MouseButton};
use raven_core::filesystem::{self, ProjectFolder};
use raven_render::{WorldRenderer};
use raven_rhi::{RHIConfig, Rhi, backend};
use raven_rg::{GraphExecutor, IntoPipelineDescriptorBindings, RenderGraphPassBindable, FrameConstants};

use raven_core::system::OnceQueue;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
pub struct EngineContext<App> {
    pub main_window: Window,
    event_loop: EventLoop<()>,
    input_manager: InputManager,

    rhi: Rhi,
    rg_executor: GraphExecutor,
    renderer: WorldRenderer,

    asset_manager: AssetManager,

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

    let render_resolution = main_window.inner_size();
    let render_resolution = [render_resolution.width, render_resolution.height];

    // create render device
    let rhi_config = RHIConfig {
        enable_debug: true,
        enable_vsync: false,
        swapchain_extent: main_window.inner_size().into(),
    };
    let rhi = Rhi::new(rhi_config, &main_window)?;

    let rg_executor = GraphExecutor::new(&rhi)?;
    let renderer = WorldRenderer::new(&rhi, render_resolution);

    let mut app = app;
    app.init()?;

    glog::trace!("Raven Engine initialized!");
    Ok(EngineContext { 
        main_window,
        event_loop,
        input_manager: InputManager::new(),

        rhi,
        rg_executor,
        renderer,

        asset_manager: AssetManager::new(),

        app,
    })
}

/// Start engine main loop.
pub fn main_loop(engine_context: &mut EngineContext<impl user::App>) {
    let EngineContext { 
        event_loop,
        main_window: _,
        input_manager,
        
        rhi,
        rg_executor,
        renderer,

        asset_manager,

        app,
    } = engine_context;

    asset_manager.load_asset(AssetLoadDesc::load_mesh("mesh/cerberus_gun/scene.gltf")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/right.jpg")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/left.jpg")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/top.jpg")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/bottom.jpg")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/front.jpg")).unwrap();
    asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/back.jpg")).unwrap();

    let handles = asset_manager.dispatch_load_tasks().unwrap();
    let tex_handles: &[Arc<AssetHandle>; 6] = handles.split_at(1).1.try_into().unwrap();

    renderer.add_cubemap_split(&rhi, tex_handles);
    let mesh_handle = renderer.add_mesh(&handles[0]);

    let xform = Affine3A::from_scale_rotation_translation(
        Vec3::splat(0.05),
        Quat::from_rotation_y(90_f32.to_radians()),
        Vec3::splat(0.0)
    );
    let _instance = renderer.add_mesh_instance(xform, mesh_handle);

    let resolution = renderer.render_resolution();
    let mut camera = camera::Camera::builder()
        .aspect_ratio(resolution[0] as f32 / resolution[1] as f32)
        .build();
    let mut camera_controller = camera::controller::FirstPersonController::new(Vec3::new(0.0, 0.5, 5.0), Quat::IDENTITY);

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

    let mut static_events = Vec::new();
    let mut last_frame_time = std::time::Instant::now();

    const FILTER_FRAME_COUNT: usize = 10;
    let mut dt_filter_queue = VecDeque::with_capacity(FILTER_FRAME_COUNT);

    let mut frame_index: u32 = 0;
    let mut persist_states = PersistStates::new();

    let mut running = true;
    while running { // main loop start
        // filter delta time to get a smooth dt for simulation and rendering
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
        let mut frame_constants = {
            let old_persist_states = persist_states.clone();

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

            camera_controller.update(&mut camera, mouse_delta,
                input_manager.is_mouse_hold(MouseButton::LEFT),
                input["walk"], input["strafe"], input["lift"]
            );
            let cam_matrices = camera.camera_matrices();

            persist_states.camera.position = camera.body.position;
            persist_states.camera.rotation = camera.body.rotation;

            // if input_manager.is_mouse_just_pressed(MouseButton::RIGHT) {
            //     display_sh_cubemap = !display_sh_cubemap;
            // }

            // user-side app tick
            app.tick(dt);

            static_events.clear();

            if persist_states.is_states_changed(&old_persist_states) {
                renderer.reset_path_tracing_accumulation();
            }

            FrameConstants {
                cam_matrices,

                frame_index,
                // TODO: this should be delayed
                pre_exposure_mult: 1.0,
                pre_exposure_prev_frame_mult: 1.0,
                pre_exposure_delta: 1.0,
            }
        };
        // tick render end
        
        // tick render begin
        {
            // prepare and compile render graph
            let prepare_result = rg_executor.prepare(|rg| {
                let main_img = renderer.prepare_rg(rg, dt);
                let exposure_state = renderer.current_exposure_state();

                frame_constants.pre_exposure_mult = exposure_state.pre_mult;
                frame_constants.pre_exposure_prev_frame_mult = exposure_state.pre_mult_prev_frame;
                frame_constants.pre_exposure_delta = exposure_state.pre_mult_delta;
    
                // copy final image to swapchain
                let mut swapchain_img = rg.get_swapchain(resolution);
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
    
                        bound_pipeline.dispatch([resolution[0], resolution[1], 1]);
    
                        Ok(())
                    });
                }
            });
    
            // draw
            match prepare_result {
                Ok(()) => {
                    rg_executor.draw(
                        &frame_constants,
                        &mut rhi.swapchain
                    );

                    frame_index = frame_index.wrapping_add(1);
                },
                Err(err) => {
                    panic!("Failed to prepare render graph with {:?}", err);
                }
            }
        } // tick render end
    } // main loop end

    glog::trace!("Exit main loop successfully!");
}

/// Shutdown raven engine.
pub fn shutdown(engine_context: EngineContext<impl user::App>) {
    let rhi = engine_context.rhi;
    rhi.device.wait_idle();

    engine_context.app.shutdown();
    engine_context.renderer.clean(&rhi);
    engine_context.rg_executor.shutdown();
    glog::trace!("Raven Engine shutdown.");
}