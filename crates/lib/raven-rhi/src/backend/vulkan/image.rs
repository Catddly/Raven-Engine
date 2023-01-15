use std::collections::HashMap;

use parking_lot::Mutex;
use ash::vk;
use derive_builder::Builder;
use vk_sync::AccessType;

use raven_math;

use super::allocator::{MemoryLocation, AllocationCreateDesc, self, Allocation};
use super::{Device, RhiError, BufferDesc, ImageBarrier};

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

#[derive(Debug)]
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
    ) -> anyhow::Result<vk::ImageView, RhiError> {
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
                level_count: view_desc.level_count.unwrap_or(image_desc.mip_levels as u32 - view_desc.base_mip_level),
                base_array_layer: 0,
                layer_count: match image_desc.image_type {
                    ImageType::Cube | ImageType::CubeArray => 6,
                    _ => 1,
                },
            })
            .build()
    }
}

pub struct ImageSubResource<'a> {
    pub data: &'a [u8],
    pub row_pitch_in_bytes: u32,
    pub base_layer: u32,
}

// implement image associated function for device
impl Device {
    pub fn create_image(
        &self,
        desc: ImageDesc,
        init_datas: Option<Vec<ImageSubResource<'_>>>
    ) -> anyhow::Result<Image, RhiError> {
        let image_ci = get_image_create_info(&desc, init_datas.is_some());

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
            .map_err(|err| RhiError::AllocationFailure {
                name: "Image".into(),
                error: err,
            })?;

        // bind memory
        unsafe {
            self.raw
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .expect("bind_image_memory")
        };

        let image = Image {
            raw: image,
            allocation: Some(allocation),
            desc,
            views: Mutex::new(HashMap::new()),
        };

        if let Some(init_datas) = init_datas {
            self.upload_image_data(&image, &[init_datas], AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer)?;
        }

        Ok(image)
    }

    pub fn upload_image_data(&self,
        image: &Image,
        init_datas: &[Vec<ImageSubResource<'_>>], // arrays of mipmaps of byte datas
        dst_access: AccessType,
    ) -> anyhow::Result<(), RhiError> {
        for (array_idx, array_datas) in init_datas.into_iter().enumerate() {
            if !array_datas.is_empty() {
                let total_init_data_bytes = array_datas.iter().map(|sub| sub.data.len()).sum::<usize>();
                let desc = &image.desc;
    
                let format_bytes: u32 = match desc.format {
                    vk::Format::R8G8B8A8_UNORM => 4,
                    vk::Format::R8G8B8A8_SRGB => 4,
                    _ => todo!("Unknown format bytes {:?}", desc.format),
                };
    
                let mut image_staging_buffer = self.create_buffer(
                    BufferDesc::new_cpu_to_gpu(total_init_data_bytes, vk::BufferUsageFlags::TRANSFER_SRC),
                    "image staging buffer"
                )?;
    
                let mut curr_offset = 0;
                let mapped_slice_mut = image_staging_buffer.allocation.mapped_slice_mut().unwrap();
    
                let buffer_copy_regions = array_datas.into_iter()
                    .enumerate()
                    .map(|(level, sub)| {
                        let width = (desc.extent[0] >> level).max(1);
                        let height = (desc.extent[1] >> level).max(1);
                        let depth = (desc.extent[2] >> level).max(1);
    
                        let data_len = sub.data.len();
    
                        assert!(data_len == ((width * height * depth) * format_bytes) as usize);
                        // copy image data
                        mapped_slice_mut[curr_offset..curr_offset + sub.data.len()].copy_from_slice(sub.data);
                        // build image copy subresource layers
                        let region = vk::BufferImageCopy::builder()
                            .buffer_offset(curr_offset as _)
                            .image_subresource(
                                vk::ImageSubresourceLayers::builder()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .base_array_layer(sub.base_layer)
                                    .layer_count(1)
                                    .mip_level(level as _)
                                    .build(),
                            )
                            .image_extent(vk::Extent3D {
                                width,
                                height,
                                depth,
                            });
    
                        curr_offset += sub.data.len();
                        region.build()
                    })
                    .collect::<Vec<_>>();
    
                self.with_setup_commands(|cb| unsafe {
                    super::barrier::image_barrier(
                        self,
                        cb,
                        &[
                            ImageBarrier::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .image(&image)
                            .discard_contents(array_idx == 0) // discard at first
                            .prev_access(&[dst_access])
                            .next_access(&[AccessType::TransferWrite])
                            .build().unwrap()
                        ]
                    );
    
                    self.raw.cmd_copy_buffer_to_image(
                        cb,
                        image_staging_buffer.raw,
                        image.raw,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &buffer_copy_regions,
                    );
    
                    super::barrier::image_barrier(
                        self,
                        cb,
                        &[
                            ImageBarrier::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .image(&image)
                            .prev_access(&[AccessType::TransferWrite])
                            .next_access(&[dst_access])
                            .build().unwrap()
                        ]
                    );
                })?;
    
                self.destroy_buffer(image_staging_buffer);
            }
        }

        Ok(())
    }

    pub fn create_image_view(
        &self,
        raw: vk::Image,
        desc: &ImageDesc,
        view_desc: &ImageViewDesc,
    ) -> anyhow::Result<vk::ImageView, RhiError> {
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

    pub fn divide_up_extent(mut self, division: [u32; 3]) -> Self {
        for (extent, div) in self.extent.iter_mut().zip(&division) {
            *extent = ((*extent + div - 1) / div).max(1);
        }
        self
    }

    pub fn divide_extent(mut self, division: [u32; 3]) -> Self {
        for (extent, div) in self.extent.iter_mut().zip(&division) {
            *extent = (*extent / div).max(1);
        }
        self
    }

    pub fn half_resolution(self) -> Self {
        self.divide_up_extent([2, 2, 2])
    }

    pub fn full_mipmap_levels(mut self) -> Self {
        self.mip_levels = raven_math::max_mipmap_level_3d(self.extent[0], self.extent[1], self.extent[2]);
        self
    }

    pub fn array_elements(mut self, num: u32) -> Self {
        self.array_elements = num;
        self
    }

    pub fn format(mut self, format: vk::Format) -> Self {
        self.format = format;
        self
    }

    pub fn create_flags(mut self, flags: vk::ImageCreateFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn usage_flags(mut self, flags: vk::ImageUsageFlags) -> Self {
        self.usage = flags;
        self
    }

    pub fn image_type(mut self, image_type: ImageType) -> Self {
        self.image_type = image_type;
        self
    }

    pub fn mipmap_level(mut self, level: u16) -> Self {
        self.mip_levels = level;
        self
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Builder)]
#[builder(pattern = "owned", derive(Clone))]
pub struct ImageViewDesc {
    /// If this is None, infer from image type
    #[builder(setter(strip_option), default)]
    pub view_type: Option<vk::ImageViewType>,
    /// If this is None, use image format for image view
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