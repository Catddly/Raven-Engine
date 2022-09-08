use std::collections::HashMap;
use std::sync::Mutex;

use ash::vk::{self};

use super::allocator::{MemoryLocation, AllocationCreateDesc};
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
        let mut views = self.views.lock().unwrap();

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
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "image",
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
            })
            .map_err(|err| RHIError::AllocationFailure {
                name: "Image".into(),
                error: err,
            })?;

        // bind memory
        // TODO: shared memory images
        unsafe {
            self.raw
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .expect("bind_image_memory")
        };

        Ok(Image {
            raw: image,
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

impl ImageDesc {

}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ImageViewDesc {
    pub view_type: Option<vk::ImageViewType>,
    pub format: Option<vk::Format>,
    pub aspect_mask: vk::ImageAspectFlags,
    pub base_mip_level: u32,
    pub level_count: Option<u32>,
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