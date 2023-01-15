use ash::vk;

use winit;

use super::platform;
use crate::backend::vulkan::Instance;

pub struct Surface {
    pub(crate) func_loader: ash::extensions::khr::Surface,
    pub(crate) raw: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(instance: &Instance, window: &winit::window::Window) -> anyhow::Result<Self> {
        let surface = unsafe { platform::create_surface(&instance.entry, &instance.raw, &window)? };

        let func_loader = ash::extensions::khr::Surface::new(&instance.entry, &instance.raw);

        Ok(Self {
            func_loader,
            raw: surface,
        })
    }
}