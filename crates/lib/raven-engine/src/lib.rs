use std::{collections::VecDeque, sync::Arc};

use ash::vk;
use turbosloth::*;

use raven_core::{
    winit::{
        event_loop::{EventLoop},
        dpi::{LogicalSize, LogicalPosition},
        window::{Window, WindowBuilder},
        event::{WindowEvent, Event, VirtualKeyCode, ElementState}, 
        platform::run_return::EventLoopExtRunReturn
    }, asset::{loader::{mesh_loader::GltfMeshLoader, AssetLoader}, AssetType, AssetProcessor}, concurrent::executor,
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::filesystem;
use raven_render::{MeshRenderer, MeshRasterScheme};
use raven_rhi::{RHIConfig, Rhi, backend::{self, PipelineShaderDesc, PipelineShaderStage, RasterPipelineDesc}};
use raven_rg::{Executor, IntoPipelineDescriptorBindings, RenderGraphPassBindable};

// Raven Engine exposed APIs
pub use raven_core::filesystem::{ProjectFolder};

use raven_core::system::OnceQueue;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
pub struct EngineContext {
    pub main_window: Window,
    event_loop: EventLoop<()>,

    rhi: Rhi,
    rg_executor: Executor,

    main_renderpass: Arc<backend::RenderPass>,
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
    let rhi = Rhi::new(rhi_config, &main_window)?;

    let rg_executor = Executor::new(&rhi)?;

    // TODO: put this inside a renderer
    let main_renderpass = backend::renderpass::create_render_pass(&rhi.device, 
        backend::renderpass::RenderPassDesc {
            color_attachments: &[backend::renderpass::RenderPassAttachmentDesc::new(vk::Format::R8G8B8A8_UNORM).useless_input()],
            depth_attachment: None,
        }
    );

    // let registry = asset_registry::get_runtime_asset_registry();

    // {
    //     let read_guard = registry.read();
    //     let asset_ref = read_guard.get_asset(&handle).unwrap();
    //     let mesh = asset_ref.as_mesh().unwrap();

    //     glog::debug!("{:?}", mesh.tangents);

    //     for mat in mesh.material_textures.iter() {
    //         let mat_v = read_guard.get_asset(mat.handle()).unwrap();
    //         let mat_v = mat_v.as_texture().unwrap();
    
    //         glog::debug!("Texture {:#?} with uuid {:8.8x}", mat_v, mat.uuid());
    //     }

    //     let mut file = std::fs::File::create("cube.packed")?;
    //     mesh.write_packed(&mut file);
    // }

    // let temp_list = TempList::new();
    // let dir = filesystem::get_project_folder_path_absolute(filesystem::ProjectFolder::Baked)?;

    // let data: &[u8] = {
    //     let path = dir.join("cube.mesh");
    //     let file = std::fs::File::open(path).unwrap();

    //     unsafe { temp_list.add(memmap2::MmapOptions::new().map(&file).unwrap()) }
    // };

    // let field_reader = Mesh::get_field_reader(data);
    
    // let readback_colors = field_reader.colors().to_vec();
    // let readback_tangents = field_reader.tangents().to_vec();
    // let readback_uvs = field_reader.uvs().to_vec();
    // let readback_material_ids = field_reader.material_ids().to_vec();
    
    // glog::debug!("{:?}", readback_colors);
    // glog::debug!("{:?}", readback_tangents);
    // glog::debug!("{:?}", readback_uvs);
    // glog::debug!("{:?}", readback_material_ids);

    // let len = field_reader.materials(VecArrayQueryParam::length()).length();
    // for i in 0..len {
    //     let disk = field_reader.materials(VecArrayQueryParam::index(i)).value();
    //     glog::debug!("{:8.8x}", disk.uuid());

    //     let data = {
    //         let path = dir.join(format!("{:8.8x}.mat", disk.uuid()));
    //         let file = std::fs::File::open(path).unwrap();
    
    //         unsafe { temp_list.add(memmap2::MmapOptions::new().map(&file).unwrap()) }
    //     };

    //     let field_reader = Material::get_field_reader(data);
    //     glog::debug!("mat metallic: {}", field_reader.metallic());
    //     glog::debug!("mat roughness: {}", field_reader.roughness());
    //     glog::debug!("mat texture mapping: {:?}", field_reader.texture_mapping());
    //     glog::debug!("mat texture transform: {:?}", field_reader.texture_transform());
    // }

    // let len = field_reader.material_textures(VecArrayQueryParam::length()).length();
    // for i in 0..len {
    //     let disk = field_reader.material_textures(VecArrayQueryParam::index(i)).value();
    //     glog::debug!("{:8.8x}", disk.uuid());

    //     let data = {
    //         let path = dir.join(format!("{:8.8x}.tex", disk.uuid()));
    //         let file = std::fs::File::open(path).unwrap();
    
    //         unsafe { temp_list.add(memmap2::MmapOptions::new().map(&file).unwrap()) }
    //     };

    //     let field_reader = Texture::get_field_reader(data);
    //     glog::debug!("tex extent: {:?}", field_reader.extent());

    //     let len = field_reader.lod_groups(VecArrayQueryParam::length()).length();
    //     for i in 0..len {
    //         let bytes = field_reader.lod_groups(VecArrayQueryParam::index(i)).array();

    //         glog::debug!("tex bytes: {:?}", bytes);
    //     }
    // }

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

    let loader = Box::new(GltfMeshLoader::new(std::path::PathBuf::from("mesh/cube.glb"))) as Box<dyn AssetLoader>;
    let raw_asset = loader.load().unwrap();

    assert!(matches!(raw_asset.asset_type(), AssetType::Mesh));

    let processor = AssetProcessor::new("mesh/cube.glb", raw_asset);
    let handle = processor.process().unwrap();
    let lazy_cache = LazyCache::create();

    let task = executor::spawn(handle.eval(&lazy_cache));
    let handle = smol::block_on(task).unwrap();

    let mut mesh_renderer = MeshRenderer::new(rhi, MeshRasterScheme::Deferred, render_resolution);
    mesh_renderer.add_asset_mesh(&handle);

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
                        .source("triangle.hlsl")
                        .stage(PipelineShaderStage::Vertex)
                        .entry("vs_main")
                        .build().unwrap(),
                    PipelineShaderDesc::builder()
                        .source("triangle.hlsl")
                        .stage(PipelineShaderStage::Pixel)
                        .entry("ps_main")
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

            mesh_renderer.prepare_rg(rg);

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
    
    //rhi.device.wait_idle();
    //mesh_renderer.clean(rhi);
    glog::trace!("Exit main loop successfully!");
}

/// Shutdown raven engine.
pub fn shutdown(engine_context: EngineContext) {
    engine_context.rg_executor.shutdown();
    glog::trace!("Raven Engine shutdown.");
}