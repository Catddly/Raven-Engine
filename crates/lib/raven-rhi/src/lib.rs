#[macro_use]
extern crate derive_builder;

pub mod backend;
pub mod pipeline_cache;
pub mod shader_compiler;
pub mod draw_frame;

use std::sync::Arc;
use raven_core::winit::window::Window;

use crate::backend::vulkan::{Instance, Surface, debug, physical_device, Device, Swapchain};

#[derive(Clone, Copy)]
pub struct RHIConfig {
    pub swapchain_extent: [u32; 2],
    pub enable_debug: bool,
    pub enable_vsync: bool,
}

// maybe raven will support RHI in the future.
// this is only a facade to vulkan
pub struct RHI {
    pub device: Arc<Device>,
    pub swapchain: Swapchain,
}

impl RHI {
    pub fn new(config: RHIConfig, window: &Window) -> anyhow::Result<Self> {
        let instance = Instance::builder().build()?;
        let surface = Arc::new(Surface::new(&instance, &window)?);

        let (_debug_util, _debug_messager) = debug::setup_debug_utils(
            config.enable_debug, 
            &instance.entry, 
            &instance.raw
        );

        let physical_device = Arc::new(physical_device::pick_suitable_physical_device(&instance, &surface));
        glog::trace!("Selected Physical Device: {:#?}", unsafe {
            std::ffi::CStr::from_ptr(physical_device.properties.device_name.as_ptr() as *const std::os::raw::c_char)
        });

        let device = Device::builder()
            .build(&physical_device)?;

        glog::trace!("Required swapchain extent: {:?}", config.swapchain_extent);
        let swapchain = Swapchain::builder()
            .extent(config.swapchain_extent)
            .enable_vsync(config.enable_vsync)
            .build(&device, &surface)?;

        Ok(Self {
            device: device.clone(),
            swapchain: swapchain,
        })
    }
}

impl Drop for RHI {
    fn drop(&mut self) {
        self.device.release_debug_resources();
    }
}

// global logger macro
extern crate log as glog;