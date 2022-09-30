use std::{collections::VecDeque, sync::Arc};

use ash::vk;

use raven_core::winit::{
    event_loop::{EventLoop},
    dpi::{LogicalSize, LogicalPosition},
    window::{Window, WindowBuilder},
    event::{WindowEvent, Event, VirtualKeyCode, ElementState}, 
    platform::run_return::EventLoopExtRunReturn,
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::filesystem;
use raven_core::asset::loader::{AssetLoader, mesh_loader};
use raven_rhi::{RHIConfig, RHI, backend::{self, PipelineShaderDesc, PipelineShaderStage, RasterPipelineDesc}};
use raven_rg::{Executor, IntoPipelineDescriptorBindings, RenderGraphPassBindable};

// Raven Engine exposed APIs
pub use raven_core::filesystem::{ProjectFolder};

use raven_core::system::OnceQueue;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
pub struct EngineContext {
    pub main_window: Window,
    event_loop: EventLoop<()>,

    rhi: RHI,
    rg_executor: Executor,

    main_renderpass: Arc<backend::RenderPass>,
}

fn init_filesystem_module() -> anyhow::Result<()> {
    filesystem::set_default_root_path()?;
    
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
pub fn init() -> anyhow::Result<EngineContext> {
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
    let rhi = RHI::new(rhi_config, &main_window)?;

    let rg_executor = Executor::new(&rhi)?;

    // Temporary
    //let loader: Box<dyn AssetLoader> = Box::new(mesh_loader::GltfMeshLoader::new(std::path::PathBuf::from("mesh/cube.glb")));
    //let loader: Box<dyn AssetLoader> = Box::new(mesh_loader::GltfMeshLoader::new(std::path::PathBuf::from("mesh/cornell_box/scene.gltf")));
    let loader: Box<dyn AssetLoader> = Box::new(mesh_loader::GltfMeshLoader::new(std::path::PathBuf::from("mesh/336_lrm/scene.gltf")));
    loader.load()?;

    // TODO: put this inside a renderer
    let main_renderpass = backend::render_pass::create_render_pass(&rhi.device, 
        backend::render_pass::RenderPassDesc {
            color_attachments: &[backend::render_pass::RenderPassAttachmentDesc::new(vk::Format::R8G8B8A8_UNORM).useless_input()],
            depth_attachment: None,
        }
    );

    glog::trace!("Raven Engine initialized!");
    Ok(EngineContext { 
        main_window,
        event_loop,

        rhi,
        rg_executor,
        main_renderpass,
    })
}

/// Start engine main loop.
pub fn main_loop(engine_context: &mut EngineContext) {
    let EngineContext { 
        event_loop,
        main_window,
        
        rhi,
        rg_executor,
        main_renderpass,
    } = engine_context;

    let mut last_frame_time = std::time::Instant::now();

    const FILTER_FRAME_COUNT: usize = 10;
    let mut dt_filter_queue = VecDeque::with_capacity(FILTER_FRAME_COUNT);

    let render_resolution = main_window.inner_size();
    let render_resolution = [render_resolution.width, render_resolution.height];

    // temporary
    let main_img_desc: backend::ImageDesc = backend::ImageDesc::new_2d(render_resolution, vk::Format::R8G8B8A8_UNORM);

    let mut running = true;
    while running { // main loop start
        // filter delta time to get a smooth result for simulation and rendering
        let _dt = {
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
                }
                Event::MainEventsCleared => {
                    control_flow.set_exit();
                }
                _ => (),
            }
        });

        // prepare and compile render graph
        let prepare_result = rg_executor.prepare(|rg| {
            // No renderer yet!
            let mut main_img = rg.new_resource(main_img_desc);

            // main rt
            {
                let main_renderpass = main_renderpass.clone();
                let mut pass = rg.add_pass("main");

                let pipeline = pass.register_raster_pipeline(&[
                    PipelineShaderDesc::builder()
                        .source("triangle_vs.hlsl")
                        .stage(PipelineShaderStage::Vertex)
                        .build().unwrap(),
                    PipelineShaderDesc::builder()
                        .source("triangle_ps.hlsl")
                        .stage(PipelineShaderStage::Pixel)
                        .build().unwrap(),
                ], 
                RasterPipelineDesc::builder()
                    .render_pass(main_renderpass.clone())
                    .culling(false)
                    .depth_write(false)
                    .build().unwrap()
                );

                let main_img_ref = pass.raster_write(&mut main_img, backend::AccessType::ColorAttachmentWrite);

                pass.render(move |context| {
                    context.begin_render_pass(&main_renderpass, render_resolution,
                        &[(main_img_ref, &backend::ImageViewDesc::default())],
                         None)?;

                    context.set_default_viewport_and_scissor(render_resolution);
                    // bind pipeline and descriptor set
                    context.bind_raster_pipeline(pipeline.into_bindings())?;

                    unsafe {
                        context.context.device.raw.cmd_draw(context.cb.raw, 
                            3, 1, 
                            0, 0);
                    }

                    context.end_render_pass();
                    Ok(())
                });
            }

            // copy to swapchain
            let mut swapchain_img = rg.get_swapchain(render_resolution);
            {
                let mut pass = rg.add_pass("final blit");
                let pipeline = pass.register_compute_pipeline("image_blit.hlsl");
                                
                let main_img_ref = pass.read(&main_img, backend::AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let swapchain_img_ref = pass.write(&mut swapchain_img, backend::AccessType::ComputeShaderWrite);

                pass.render(move|context| {
                    // bind pipeline and descriptor set
                    let bound_pipeline = context.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            main_img_ref.bind(), 
                            swapchain_img_ref.bind()
                        ])
                    )?;

                    bound_pipeline.dispatch([render_resolution[0], render_resolution[1], 0]);

                    Ok(())
                });
            }
        });

        // draw
        match prepare_result {
            Ok(()) => {
                rg_executor.draw(&mut rhi.swapchain);
            },
            Err(err) => {
                panic!("Failed to prepare render graph with {:?}", err);
            }
        }
    } // main loop end
    
    glog::trace!("Exit main loop successfully!");
}

/// Shutdown raven engine.
pub fn shutdown(engine_context: EngineContext) {
    engine_context.rg_executor.shutdown();
    glog::trace!("Raven Engine shutdown.");
}