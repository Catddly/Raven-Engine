use std::sync::{Arc};
use std::ops::Deref;

use parking_lot::RwLock;
use winit::{dpi::{LogicalSize, LogicalPosition}, window::WindowBuilder};
use winit::{window::Window, event_loop::EventLoop};

#[non_exhaustive]
pub struct CoreApiInner {
    event_loop: EventLoop<()>,
    main_window: Window,
}

impl std::fmt::Debug for CoreApiInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug CoreApiInner")
    }
}

impl CoreApiInner {
    pub fn new() -> Self {
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

        let main_window = WindowBuilder::new()
            .with_inner_size(window_resolution)
            .with_position(window_position)
            .with_resizable(false)
            .with_title("Raven Engine")
            .build(&event_loop)
            .expect("Failed to create a window!");

        Self {
            event_loop,
            main_window,
        }
    }

    #[inline]
    pub fn main_window(&self) -> &Window {
        &self.main_window
    }

    #[inline]
    pub fn event_loop(&self) -> &EventLoop<()> {
        &self.event_loop
    }

    #[inline]
    pub fn event_loop_mut(&mut self) -> &mut EventLoop<()> {
        &mut self.event_loop
    }
}

#[derive(Clone)]
pub struct CoreApiImpl(Option<Arc<RwLock<CoreApiInner>>>);

unsafe impl Send for CoreApiImpl {}
unsafe impl Sync for CoreApiImpl {}

impl std::fmt::Debug for CoreApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug Default CoreApiImpl")
    }
}

impl Deref for CoreApiImpl {
    type Target = Arc<RwLock<CoreApiInner>>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl CoreApiImpl {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn init(&mut self) {
        self.0 = Some(Arc::new(RwLock::new(CoreApiInner::new())));
    }

    pub fn shutdown(mut self) {
        if let Some(inner) = self.0.take() {
            let inner = Arc::try_unwrap(inner)
                .expect("Reference counting of core api may not be retained!");
            let inner = inner.into_inner();
            drop(inner);
        } else {
            panic!("Try to shutdown core apis before initializing!");
        }
    }
}
