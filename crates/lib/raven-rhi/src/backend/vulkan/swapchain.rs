use std::{sync::Arc, ops::{Deref, DerefMut}};

use ash::vk;

use super::{Surface, Device, Image, ImageDesc, RhiError};

pub struct SwapchainImage {
    pub image: Arc<Image>,
    pub acquire_semaphore: vk::Semaphore,
    pub render_finished_semaphore: vk::Semaphore,
    pub frame_index: u32,
}

impl Deref for SwapchainImage {
    type Target = Arc<Image>;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl DerefMut for SwapchainImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.image
    }
}

pub struct Swapchain {
    pub(crate) raw: vk::SwapchainKHR,
    pub(crate) func_loader: ash::extensions::khr::Swapchain,

    pub images: Vec<Arc<Image>>,
    pub acquire_semaphores: Vec<vk::Semaphore>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub current_frame: u32,
    
    // since instance and physical device are only valid if and only if device is valid,
    // so keep a atomic reference counter here to avoid incorrect dropping.
    // for convenience purpose, too.
    // aka. Aggregate Design
    pub(crate) device: Arc<Device>,
    #[allow(dead_code)]
    pub(crate) surface: Arc<Surface>,

    pub extent: vk::Extent2D,
    pub enable_vsync: bool,
}

impl Swapchain {
    pub fn builder() -> SwapchainBuilder {
        Default::default()
    }

    pub fn acquire_next_image(&mut self) -> anyhow::Result<SwapchainImage, RhiError> {
        let current_frame = &mut self.current_frame;
        let acquire_semaphore = self.acquire_semaphores[*current_frame as usize];

        unsafe {
            match self.func_loader.acquire_next_image(self.raw, std::u64::MAX, acquire_semaphore, vk::Fence::null()) {
                Ok((idx, _)) => { 
                    assert_eq!(idx, *current_frame);

                    *current_frame = (*current_frame + 1) % (self.images.len() as u32);
                    Ok(SwapchainImage {
                        image: self.images[idx as usize].clone(),
                        acquire_semaphore: acquire_semaphore,
                        render_finished_semaphore: self.render_finished_semaphores[idx as usize],
                        frame_index: idx,
                    })
                },
                Err(err) if err == vk::Result::ERROR_OUT_OF_DATE_KHR ||
                    err == vk::Result::SUBOPTIMAL_KHR => {
                    Err(RhiError::FramebufferInvalid)
                }
                Err(err) => {
                    Err(RhiError::AcquiredImageFailed { err })
                }
            }
        }
    }

    pub fn present(&self, image: SwapchainImage) {
        let present_info = vk::PresentInfoKHR::builder()
            .image_indices(std::slice::from_ref(&image.frame_index))
            .swapchains(std::slice::from_ref(&self.raw))
            .wait_semaphores(std::slice::from_ref(&image.render_finished_semaphore))
            .build();

        let result = unsafe {
            self.func_loader
                .queue_present(self.device.global_queue.raw, &present_info)      
        };

        match result {
            Ok(_) => {},
            Err(err) if err == vk::Result::ERROR_OUT_OF_DATE_KHR ||
                err == vk::Result::SUBOPTIMAL_KHR => { /* handle this when acquiring image in the next frame */ }
            _ => {
                panic!("Vulkan Failed on presenting image!");
            }
        }
    }

    fn enumerate_available_surface_format(device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<Vec<vk::SurfaceFormatKHR>> {
        unsafe {
            Ok(surface
                .func_loader
                .get_physical_device_surface_formats(device.physical_device.raw, surface.raw)?)
        }
    }

    fn enumerate_available_surface_capabilities(device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<vk::SurfaceCapabilitiesKHR> {
        unsafe {
            Ok(surface
                .func_loader
                .get_physical_device_surface_capabilities(device.physical_device.raw, surface.raw)?)
        }
    }

    fn enumerate_available_surface_present_modes(device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<Vec<vk::PresentModeKHR>> {
        unsafe {
            Ok(surface
                .func_loader
                .get_physical_device_surface_present_modes(device.physical_device.raw, surface.raw)?)
        }
    }

    fn pick_suitable_surface_format(device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<vk::SurfaceFormatKHR> {
        let surface_formats = Self::enumerate_available_surface_format(&device, &surface)?;

        let pick_surface_formats = match surface_formats.len() {
            0 => unreachable!(),
            // if there is only one format with vk::Format::UNDEFINED,
            // there is no preferred format, so we assume VK_FORMAT_B8G8R8A8_UNORM
            1 if surface_formats[0].format == vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_UNORM,
                color_space: surface_formats[0].color_space,
            },
            _ => {
                surface_formats.iter()
                    // prefer format VK_FORMAT_B8G8R8A8_UNORM
                    .find(|format| format.format == vk::Format::B8G8R8A8_UNORM)
                    // if prefer format is not available, pick the first one,
                    .unwrap_or(&surface_formats[0])
                    .clone()
            }
        };

        Ok(pick_surface_formats)
    }

    fn new(builder: SwapchainBuilder, device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<Self> {
        let surface_capabilities = Self::enumerate_available_surface_capabilities(&device, &surface)?;

        // triple-buffering for swapchain images
        let image_count = 3.max(surface_capabilities.min_image_count);
        assert!(image_count <= surface_capabilities.max_image_count);

        let image_resolution = match surface_capabilities.current_extent.width {
            std::u32::MAX => builder.extent,
            _ => surface_capabilities.current_extent,
        };

        if 0 == image_resolution.width || 0 == image_resolution.height {
            anyhow::bail!("Swapchain resolution can NOT be zero!");
        }

        // choose present modes by vsync, the one at the front will be chosen first if they both supported by the surface.
        // more info: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html
        let present_modes = if builder.enable_vsync {
            vec![vk::PresentModeKHR::FIFO_RELAXED, vk::PresentModeKHR::FIFO]
        } else {
            vec![vk::PresentModeKHR::MAILBOX, vk::PresentModeKHR::IMMEDIATE]
        };

        let surface_supported_present_modes = Self::enumerate_available_surface_present_modes(&device, &surface)?;

        let present_mode = present_modes.into_iter()
            .find(|pm| surface_supported_present_modes.contains(pm))
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let surface_transform = if surface_capabilities.supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        let surface_format = Self::pick_suitable_surface_format(&device, &surface)
            .expect("Failed to pick a suitable surface format!");

        let swapchain_ci = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface.raw)
            .min_image_count(image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(image_resolution)
            .image_usage(vk::ImageUsageFlags::STORAGE) // storage or color_attachment?
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1)
            .build();

        let func_loader = ash::extensions::khr::Swapchain::new(&device.instance.raw, &device.raw);
        let swapchain = unsafe { func_loader.create_swapchain(&swapchain_ci, None) }
            .expect("Failed to create swapchain!");
        glog::trace!("Vulkan swapchain created!");

        // fetch images from swapchain
        let raw_images = unsafe { func_loader.get_swapchain_images(swapchain) }.expect("Failed to get swapchain images!");

        // directly construct image
        let images: Vec<_> = raw_images.into_iter()
            .map(|raw| 
                Arc::new(Image {
                    raw: raw,
                    allocation: None,
                    desc: ImageDesc::new_2d([builder.extent.width, builder.extent.height], vk::Format::B8G8R8A8_UNORM)
                        .usage_flags(vk::ImageUsageFlags::STORAGE),
                    views: Default::default(),
            }))
            .collect();
        assert_eq!(images.len() as u32, image_count);

        // create image views
        // for image in &mut images {
        //     image.view(&device, &ImageViewDesc {
        //         view_type: None,
        //         format: None,
        //         aspect_mask: vk::ImageAspectFlags::COLOR,
        //         base_mip_level: 0,
        //         level_count: Some(1),
        //     })
        //     .expect("Failed to create image view for swapchain images!");
        // }

        let acquire_semaphores = (0..images.len()).into_iter()
            .map(|_| unsafe { 
                device.raw
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
                }
            )
            .collect();

        let render_finished_semaphores = (0..images.len()).into_iter()
            .map(|_| unsafe { 
                device.raw
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
                }
            )
            .collect();

        Ok(Self {
            raw: swapchain,
            func_loader,
            extent: builder.extent,
            enable_vsync: builder.enable_vsync,

            images,
            acquire_semaphores,
            render_finished_semaphores,
            current_frame: 0,

            device: device.clone(),
            surface: surface.clone(),
        })
    }
}

pub struct SwapchainBuilder {
    pub extent: vk::Extent2D,
    pub enable_vsync: bool,
}

impl Default for SwapchainBuilder {
    fn default() -> Self {
        Self {
            // TODO: this is not the same as outside in the window setups
            extent: vk::Extent2D {
                width: 0,
                height: 0,
            },
            enable_vsync: false,
        }
    }
}

impl SwapchainBuilder {
    pub fn extent(mut self, extent: [u32; 2]) -> Self {
        self.extent = vk::Extent2D::builder()
            .width(extent[0])
            .height(extent[1])
            .build();
        self
    }

    pub fn enable_vsync(mut self, enable_vsync: bool) -> Self {
        self.enable_vsync = enable_vsync;
        self
    }

    pub fn build(self, device: &Arc<Device>, surface: &Arc<Surface>) -> anyhow::Result<Swapchain> {
        Ok(Swapchain::new(self, device, surface)?)
    }
}