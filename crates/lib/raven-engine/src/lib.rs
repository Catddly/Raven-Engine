use anyhow::{Ok};
use winit::{
    event_loop::{EventLoop},
    dpi::{LogicalSize},
    window::{Window, WindowBuilder},
    event::{WindowEvent, Event, VirtualKeyCode}, 
    platform::run_return::EventLoopExtRunReturn,
};

extern crate log as glog;

// Raven Engine APIs
use raven_core::log;
use raven_core::console;
use raven_core::filesystem;

// Raven Engine exposed APIs
pub use raven_core::filesystem::{ProjectFolder};

/// Global engine context to have control on engine on user side.
/// Facade Design Pattern to take control on different part of engine without knowing the underlying implementation.
pub struct EngineContext {
    pub main_window: Window,
    event_loop: EventLoop<()>,
}

/// Initialize raven engine.
pub fn init() -> anyhow::Result<EngineContext> {
    filesystem::set_default_root_path()?;

    let console_var = console::from_args();

    log::init_log(log::LogConfig {
        level: console_var.level,
    }).expect("Failed to initialize module log!");

    // init event loop
    let event_loop = EventLoop::new();
    event_loop.
        primary_monitor()
        .expect("Must have at least one monitor!")
        .video_modes()
        .next()
        .expect("Must have at least one video modes!");

    // create main window
    let main_window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(
            1080.0,
            720.0
        ))
        .with_title("Raven Engine")
        .build(&event_loop)
        .expect("Failed to create a window!");

    glog::trace!("Raven Engine initialized!");
    Ok(EngineContext { 
        main_window, 
        event_loop, 
    })
}

/// Start engine main loop.
pub fn main_loop(engine_context: &mut EngineContext) {
    glog::trace!("Begin main loop.");
    let EngineContext { 
        main_window: _,
        event_loop 
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
                        if let Some(VirtualKeyCode::Escape) = input.virtual_keycode {
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
    glog::trace!("Raven Engine shutdown.");
}