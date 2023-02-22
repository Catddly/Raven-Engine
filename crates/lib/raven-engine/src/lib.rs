extern crate log as glog;

mod user;
pub mod prelude;

use std::{collections::VecDeque, sync::Arc};

use winit::{
    event::{WindowEvent, Event, ElementState, VirtualKeyCode},
    platform::run_return::EventLoopExtRunReturn
};

use raven_facade::{log, input, render::{LightFrameConstants, FrameConstants}};
use raven_facade::math::{Vec3, Quat, Affine3A};
use raven_facade::asset::{AssetManager, AssetLoadDesc, asset_registry::AssetHandle};
use raven_facade::scene::{camera::{self}, persistence::{PersistStates, IsStatesChanged}};
use raven_facade::input::{InputApi, KeyCode, InputBinding, MouseButton};

use raven_facade::core::{self, console, CoreApi};
use raven_facade::filesystem::{self, ProjectFolder};

use raven_facade::render::{self, RenderApi};
#[cfg(feature = "gpu_ray_tracing")]
use raven_facade::render::RenderMode;

static mut ENGINE_CONTEXT: Option<EngineContext> = None;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
struct EngineContext {
    core_api: CoreApi,
    input_api: InputApi,
    render_api: RenderApi,

    asset_manager: AssetManager,

    app: Box<dyn user::App>,
}

fn init_filesystem() -> anyhow::Result<()> {
    filesystem::set_default_root_path()?;
    
    filesystem::set_custom_mount_point(ProjectFolder::Baked, "../../resource/baked/")?;
    filesystem::set_custom_mount_point(ProjectFolder::Assets, "../../resource/assets/")?;
    filesystem::set_custom_mount_point(ProjectFolder::ShaderSource, "../../resource/shader_src/")?;

    Ok(())
}

fn init_log() -> anyhow::Result<()> {
    let console_var = console::from_args();

    log::init_log(log::LogConfig {
        level: console_var.level,
    })?;

    Ok(())
}

/// Initialize raven engine.
pub fn init(app: Box<dyn user::App>) -> anyhow::Result<()> {
    init_filesystem()?;
    init_log()?;

    let core_api = core::CoreApi::new();
    let input_api = input::InputApi::new();
    let render_api = render::RenderApi::new();

    let mut app = app;
    app.init()?;

    glog::trace!("Raven Engine initialized!");
    unsafe {
        ENGINE_CONTEXT = Some(EngineContext { 
            core_api,
            input_api,
            render_api,
    
            asset_manager: AssetManager::new(),
    
            app,
        });
    
        if let Some(ctx) = &mut ENGINE_CONTEXT {
            ctx.core_api.init();
            core::connect(&mut ctx.core_api);

            ctx.input_api.init();
            input::connect(&mut ctx.input_api);

            ctx.render_api.init();
            render::connect(&mut ctx.render_api);
        }
    }

    Ok(())
}

/// Start engine main loop.
pub fn main_loop() {
    unsafe {
        let EngineContext {
            core_api: _,
            input_api,
            render_api,

            asset_manager,

            app,
        } = ENGINE_CONTEXT.as_mut().unwrap();

        asset_manager.load_asset(AssetLoadDesc::load_mesh("mesh/cerberus_gun/scene.gltf")).unwrap();
        //asset_manager.load_asset(AssetLoadDesc::load_mesh("mesh/cornell_box/scene.gltf")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/right.jpg")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/left.jpg")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/top.jpg")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/bottom.jpg")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/front.jpg")).unwrap();
        asset_manager.load_asset(AssetLoadDesc::load_texture("texture/skybox/back.jpg")).unwrap();

        let handles = asset_manager.dispatch_load_tasks().unwrap();
        let tex_handles: &[Arc<AssetHandle>; 6] = handles.split_at(1).1.try_into().unwrap();

        {
            let mut render_api = render_api.write(); 
            render_api.add_cubemap_split(tex_handles);
            let mesh_handle = render_api.add_mesh(&handles[0]);

            let gun_xform = Affine3A::from_scale_rotation_translation(
                Vec3::splat(0.05),
                Quat::from_rotation_y(90_f32.to_radians()),
                Vec3::splat(0.0)
            );
            // let cornell_xform = Affine3A::from_scale_rotation_translation(
            //     Vec3::splat(1.0),
            //     Quat::IDENTITY,
            //     Vec3::splat(0.0)
            // );

            let _instance = render_api.add_mesh_instance(mesh_handle, gun_xform);

            let resolution = render_api.get_render_resolution();
            let camera = camera::Camera::builder()
                .aspect_ratio(resolution[0] as f32 / resolution[1] as f32)
                .build();
            let camera_controller = camera::controller::FirstPersonController::new(Vec3::new(0.0, 0.5, 5.0), Quat::IDENTITY);

            render_api.set_main_camera(camera, camera_controller);
        }

        {
            let mut input_api = input_api.write();
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::W), 
                InputBinding::new("walk", 1.0).activation_time(0.2)
            );
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::S), 
                InputBinding::new("walk", -1.0).activation_time(0.2)
            );
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::A), 
                InputBinding::new("strafe", -1.0).activation_time(0.2)
            );
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::D), 
                InputBinding::new("strafe", 1.0).activation_time(0.2)
            );
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::Q), 
                InputBinding::new("lift", -1.0).activation_time(0.2)
            );
            input_api.add_binding(
                KeyCode::vkcode(VirtualKeyCode::E), 
                InputBinding::new("lift", 1.0).activation_time(0.2)
            );
            drop(input_api);
        }

        let mut static_events = Vec::new();
        
        let mut last_frame_time = std::time::Instant::now();
        const FILTER_FRAME_COUNT: usize = 10;
        let mut dt_filter_queue = VecDeque::with_capacity(FILTER_FRAME_COUNT);

        //let mut frame_index: u32 = 0;
        let mut persist_states = PersistStates::new();

        #[cfg(feature = "gpu_ray_tracing")]
        let mut use_reference_mode = false;

        let mut running = true;
        // main loop start
        while running {
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
            let frame_constants = {
                let old_persist_states = persist_states.clone();

                // collect system messages
                {
                    let mut core_api = core::get().write();

                    let event_loop = core_api.event_loop_mut();

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
                }

                let cam_matrices = {
                    let mut input_api = input_api.write();
                    let mut render_api = render_api.write();

                    input_api.update(&static_events);
                    let input = input_api.map(dt);
                    let mouse_delta = input_api.mouse_pos_delta() * dt;
        
                    // TODO: update this using event system
                    render_api.update_camera(
                        mouse_delta, input_api.is_mouse_button_hold(MouseButton::LEFT), &input
                    );
                    let cam_matrices = render_api.get_camera_render_data();
        
                    persist_states.camera.position = render_api.get_camera_position();
                    persist_states.camera.rotation = render_api.get_camera_rotation();
        
                    #[cfg(feature = "gpu_ray_tracing")]
                    if input_api.is_keyboard_just_pressed(VirtualKeyCode::T) {
                        use_reference_mode = !use_reference_mode;
        
                        if use_reference_mode {
                            render_api.set_render_mode(RenderMode::GpuPathTracing);
                        } else {
                            render_api.set_render_mode(RenderMode::Raster);
                        }
                    }

                    cam_matrices
                };

                // user-side app tick
                app.tick_logic(dt);

                static_events.clear();

                if persist_states.is_states_changed(&old_persist_states) {
                    #[cfg(feature = "gpu_ray_tracing")]
                    render_api.write().reset_path_tracing_accumulation();
                }

                let mut light_constants: [LightFrameConstants; 10] = Default::default();
                light_constants[0] = LightFrameConstants {
                    color: [1.0, 1.0, 1.0],
                    shadowed: 1, // true
                    direction: [-0.32803, 0.90599, 0.26749],
                    intensity: 1.0
                };

                FrameConstants {
                    cam_matrices,
                    light_constants,

                    frame_index: render_api.read().current_frame_index(),
                    // TODO: this should be delayed
                    pre_exposure_mult: 1.0,
                    pre_exposure_prev_frame_mult: 1.0,
                    pre_exposure_delta: 1.0,

                    // TODO: add scene, no hardcode here
                    directional_light_count: 1,
                    pad0: 0,
                    pad1: 0,
                    pad2: 0,
                }
            };
            // tick render end
            
            // tick render begin
            render_api.write().prepare_frame(dt);
            render_api.write().draw_frame(frame_constants);
            // tick render end
        } // main loop end

        glog::trace!("Exit main loop successfully!");
    }
}

/// Shutdown raven engine.
pub fn shutdown() {
    unsafe {
        if let Some(engine_ctx) = ENGINE_CONTEXT.take() {
            let EngineContext {
                core_api,
                input_api,
                render_api,
    
                asset_manager: _,
    
                mut app,
            } = engine_ctx;

            render_api.read().device_wait_idle();

            app.shutdown();

            render_api.shutdown();
            input_api.shutdown();
            core_api.shutdown();
        }
    }

    glog::trace!("Raven Engine shutdown.");
}