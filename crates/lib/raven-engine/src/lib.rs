use anyhow::{Ok};
use raven_core::winit::{
    event_loop::{EventLoop},
    dpi::{LogicalSize},
    window::{Window, WindowBuilder},
    event::{WindowEvent, Event, VirtualKeyCode, ElementState}, 
    platform::run_return::EventLoopExtRunReturn,
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::filesystem;
use raven_rhi::{RHIConfig, RHI};

// Raven Engine exposed APIs
pub use raven_core::filesystem::{ProjectFolder};

use raven_core::system::OnceQueue;

/// Global engine context to control engine on the user side.
/// Facade Design Pattern to control different parts of engine without knowing the underlying implementation.
pub struct EngineContext {
    pub main_window: Window,
    pub rhi: RHI,

    event_loop: EventLoop<()>,
}

fn init_filesystem_module() -> anyhow::Result<()> {
    filesystem::set_default_root_path()?;
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

    init_queue.push_job(init_filesystem_module); // init filesystem
    init_queue.push_job(init_log_module); // init log

    init_queue.execute()?;

    // init event loop
    let event_loop = EventLoop::new();
    event_loop.primary_monitor()
        .expect("Must have at least one monitor!")
        .video_modes()
        .next()
        .expect("Must have at least one video modes!");

    // create main window
    let main_window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(
            1280.0,
            720.0
        ))
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

    glog::trace!("Raven Engine initialized!");
    Ok(EngineContext { 
        main_window,
        event_loop,
        rhi,
    })
}

/// Start engine main loop.
pub fn main_loop(engine_context: &mut EngineContext) {
    let EngineContext { 
        event_loop,
        ..
    } = engine_context;

    let mut running = true;
    while running {
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
                _ => (),
            }
        });
    }
    
    glog::trace!("Exit main loop successfully!");
}

/// Shutdown raven engine.
pub fn shutdown(_engine_context: EngineContext) {
    // here will have a OnceQueue to shutdown.
    glog::trace!("Raven Engine shutdown.");
}