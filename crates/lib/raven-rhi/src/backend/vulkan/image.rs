use std::collections::HashMap;

use parking_lot::Mutex;
use ash::vk::{self};
use derive_builder::Builder;

use super::allocator::{MemoryLocation, AllocationCreateDesc, self, Allocation};
use super::{Device, RHIError};

// image type is associated with image view type.
// use this for both types.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ImageType {
    Tex1d = 0,
    Tex1dArray = 1,
    Tex2d = 2,
    Tex2dArray = 3,
    Tex3d = 4,
    Cube = 5,
    CubeArray = 6,
}

pub fn image_type_to_view_type(image_type: ImageType) -> vk::ImageViewType {
    match image_type {
        ImageType::Tex1d => vk::ImageViewType::TYPE_1D,
        ImageType::Tex1dArray => vk::ImageViewType::TYPE_1D_ARRAY,
        ImageType::Tex2d => vk::ImageViewType::TYPE_2D,
        ImageType::Tex2dArray => vk::ImageViewType::TYPE_2D_ARRAY,
        ImageType::Tex3d => vk::ImageViewType::TYPE_3D,
        ImageType::Cube => vk::ImageViewType::CUBE,
        ImageType::CubeArray => vk::ImageViewType::CUBE_ARRAY,
    }
}

pub struct Image {
    pub raw: vk::Image,
    // TODO: shared memory images
    // why Option? because swapchain image doesn't have Allocation, but we want a unified representation of Image.
    pub allocation: Option<Allocation>,
    pub desc: ImageDesc,
    pub views: Mutex<HashMap<ImageViewDesc, vk::ImageView>>,
}

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

impl Image {
    /// Get or Create a new image view for itself
    pub fn view(
        &self,
        device: &Device,
        view_desc: &ImageViewDesc,
    ) -> anyhow::Result<vk::ImageView, RHIError> {
        let mut views = self.views.lock();

        if let Some(view) = views.get(view_desc) {
            Ok(*view)
        } else {
            let view = device.create_image_view(self.raw, &self.desc, &view_desc)?;
            Ok(*views.entry(*view_desc).or_insert(view))
        }
    }

    pub fn view_create_info(&self, view_desc: &ImageViewDesc) -> vk::ImageViewCreateInfo {
        Self::populate_view_create_info(&self.desc, &view_desc)
    }

    fn populate_view_create_info(image_desc: &ImageDesc, view_desc: &ImageViewDesc) -> vk::ImageViewCreateInfo {
        vk::ImageViewCreateInfo::builder()
            .format(view_desc.format.unwrap_or(image_desc.format))
            // no swizzle
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            })
            .view_type(
                view_desc.view_type
                    .unwrap_or_else(|| image_type_to_view_type(image_desc.image_type)),
            )
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: view_desc.aspect_mask,
                base_mip_level: view_desc.base_mip_level,
                level_count: view_desc.level_count.unwrap_or(image_desc.mip_levels as u32),
                base_array_layer: 0,
                layer_count: match image_desc.image_type {
                    ImageType::Cube | ImageType::CubeArray => 6,
                    _ => 1,
                },
            })
            .build()
    }
}

// implement image associated function for device
impl Device {
    pub fn create_image(
        &self,
        desc: ImageDesc 
    ) -> anyhow::Result<Image, RHIError> {
        let image_ci = get_image_create_info(&desc, false);

        let image = unsafe {
            self.raw
                .create_image(&image_ci, None)
                .expect("Failed to create vulkan image!")
        };

        let requirements = unsafe { self.raw.get_image_memory_requirements(image) };

        let allocation = self.global_allocator
            .lock()
            .allocate(&AllocationCreateDesc {
                name: "image",
                requirements,
                location: allocator::to_inner_memory_location(&MemoryLocation::GpuOnly),
                linear: false,
            })
            .map_err(|err| RHIError::AllocationFailure {
                name: "Image".into(),
                error: err,
            })?;

        // bind memory
        unsafe {
            self.raw
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .expect("bind_image_memory")
        };

        Ok(Image {
            raw: image,
            allocation: Some(allocation),
            desc,
            views: Mutex::new(HashMap::new()),
        })
    }

    pub fn create_image_view(
        &self,
        raw: vk::Image,
        desc: &ImageDesc,
        view_desc: &ImageViewDesc,
    ) -> anyhow::Result<vk::ImageView, RHIError> {
        // if image_desc.format == vk::Format::D32_SFLOAT && !desc.aspect_mask.contains(vk::ImageAspectFlags::DEPTH)
        // {
        //     return Err(BackendError::ResourceAccess {
        //         info: "Depth-only resource used without the vk::ImageAspectFlags::DEPTH flag"
        //             .to_owned(),
        //     });
        // }

        let create_info = vk::ImageViewCreateInfo {
            image: raw,
            ..Image::populate_view_create_info(&desc, &view_desc)
        };

        Ok(unsafe { self.raw.create_image_view(&create_info, None)? })
    }

    pub fn destroy_image(&self, image: Image) {
        let views = image.views.into_inner();
        for (_, view) in views {
            unsafe {
                self.raw
                    .destroy_image_view(view, None);
            }
        }

        if let Some(alloc) = image.allocation {
            self.global_allocator.lock().free(alloc).expect("Failed to free vulkan image memory!");
        }

        unsafe {
            self.raw
                .destroy_image(image.raw, None);
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ImageDesc {
    pub extent: [u32; 3],
    pub image_type: ImageType,
    pub usage: vk::ImageUsageFlags,
    pub flags: vk::ImageCreateFlags,
    pub format: vk::Format,
    pub sample: vk::SampleCountFlags,
    pub tiling: vk::ImageTiling,
    pub array_elements: u32,
    pub mip_levels: u16,
}

impl Default for ImageDesc {
    fn default() -> Self {
        Self {
            extent: [0, 0, 0],
            format: vk::Format::UNDEFINED,
            image_type: ImageType::Tex2d,
            // we can infer usage by its AccessType, so user do not need to explicitly fill in here
            // but we still give user choice to add usage flags if needed
            usage: vk::ImageUsageFlags::default(),
            flags: vk::ImageCreateFlags::empty(),
            sample: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            array_elements: 1,
            mip_levels: 1,
        }
    }
}

impl ImageDesc {  
    pub fn new_1d(extent: u32, format: vk::Format) -> Self {
        Self {
            extent: [extent, 1, 1],
            format,
            image_type: ImageType::Tex1d,
            ..Default::default()
        }
    }

    pub fn new_1d_array(extent: u32, format: vk::Format, array_elements: u32) -> Self {
        Self::new_1d(extent, format).array_elements(array_elements).image_type(ImageType::Tex1dArray)
    }

    pub fn new_2d(extent: [u32; 2], format: vk::Format) -> Self {
        Self {
            extent: [extent[0], extent[1], 1],
            format,
            image_type: ImageType::Tex2d,
            ..Default::default()
        }
    }

    pub fn new_2d_array(extent: [u32; 2], format: vk::Format, array_elements: u32) -> Self {
        Self::new_2d(extent, format).array_elements(array_elements).image_type(ImageType::Tex2dArray)
    }

    pub fn new_3d(extent: [u32; 3], format: vk::Format) -> Self {
        Self {
            extent,
            format,
            image_type: ImageType::Tex3d,
            ..Default::default()
        }
    }

    pub fn new_cube(extent: u32, format: vk::Format) -> Self {
        Self {
            extent: [extent, extent, 1],
            format,
            image_type: ImageType::Cube,
            ..Default::default()
        }.array_elements(6).create_flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
    }
    
    #[inline]
    pub fn array_elements(mut self, num: u32) -> Self {
        self.array_elements = num;
        self
    }

    #[inline]
    pub fn create_flags(mut self, flags: vk::ImageCreateFlags) -> Self {
        self.flags = flags;
        self
    }

    #[inline]
    pub fn usage_flags(mut self, flags: vk::ImageUsageFlags) -> Self {
        self.usage = flags;
        self
    }

    #[inline]
    pub fn image_type(mut self, image_type: ImageType) -> Self {
        self.image_type = image_type;
        self
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Builder)]
#[builder(pattern = "owned", derive(Clone))]
pub struct ImageViewDesc {
    /// If this is None, infer from image type
    #[builder(setter(strip_option), default)]
    pub view_type: Option<vk::ImageViewType>,
    /// If this is None, use same image format for image view
    #[builder(setter(strip_option), default)]
    pub format: Option<vk::Format>,
    #[builder(default = "vk::ImageAspectFlags::COLOR")]
    pub aspect_mask: vk::ImageAspectFlags,
    #[builder(default = "0")]
    pub base_mip_level: u32,
    #[builder(default = "None")]
    pub level_count: Option<u32>,
}

impl ImageViewDesc {
    pub fn builder() -> ImageViewDescBuilder {
        Default::default()
    }
}

impl Default for ImageViewDesc {
    fn default() -> Self {
        ImageViewDescBuilder::default().build().unwrap()
    }
}

pub fn get_image_create_info(desc: &ImageDesc, with_initial_data: bool) -> vk::ImageCreateInfo {
    let (image_type, image_extent, image_layers) = match desc.image_type {
        ImageType::Tex1d => (
            vk::ImageType::TYPE_1D,
            vk::Extent3D {
                width: desc.extent[0],
                height: 1,
                depth: 1,
            },
            1,
        ),
        ImageType::Tex1dArray => (
            vk::ImageType::TYPE_1D,
            vk::Extent3D {
                width: desc.extent[0],
                height: 1,
                depth: 1,
            },
            desc.array_elements,
        ),
        ImageType::Tex2d => (
            vk::ImageType::TYPE_2D,
            vk::Extent3D {
                width: desc.extent[0],
                height: desc.extent[1],
                depth: 1,
            },
            1,
        ),
        ImageType::Tex2dArray => (
            vk::ImageType::TYPE_2D,
            vk::Extent3D {
                width: desc.extent[0],
                height: desc.extent[1],
                depth: 1,
            },
            desc.array_elements,
        ),
        ImageType::Tex3d => (
            vk::ImageType::TYPE_3D,
            vk::Extent3D {
                width: desc.extent[0],
                height: desc.extent[1],
                depth: desc.extent[2] as u32,
            },
            1,
        ),
        ImageType::Cube => (
            vk::ImageType::TYPE_2D,
            vk::Extent3D {
                width: desc.extent[0],
                height: desc.extent[1],
                depth: 1,
            },
            6,
        ),
        ImageType::CubeArray => (
            vk::ImageType::TYPE_2D,
            vk::Extent3D {
                width: desc.extent[0],
                height: desc.extent[1],
                depth: 1,
            },
            6 * desc.array_elements,
        ),
    };

    let mut image_usage = desc.usage;
    // need to copy bytes from CPU to GPU
    if with_initial_data {
        image_usage |= vk::ImageUsageFlags::TRANSFER_DST;
    }

    vk::ImageCreateInfo {
        flags: desc.flags,
        image_type,
        format: desc.format,
        extent: image_extent,
        mip_levels: desc.mip_levels as u32,
        array_layers: image_layers as u32,
        samples: desc.sample,
        tiling: desc.tiling,
        usage: image_usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: match with_initial_data {
            true => vk::ImageLayout::PREINITIALIZED,
            false => vk::ImageLayout::UNDEFINED,
        },
        ..Default::default()
    }
}