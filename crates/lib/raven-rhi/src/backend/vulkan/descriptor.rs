use std::collections::{btree_map, BTreeMap};
#[cfg(feature = "gpu_ray_tracing")]
use std::cell::UnsafeCell;

use raven_container::TempList;
use rspirv_reflect::{DescriptorInfo, DescriptorType, BindingCount};
use ash::vk;

use crate::{backend::{ImageViewDesc, SamplerDesc}};

use super::{Device, Buffer, Image, pipeline::{CommonPipeline, PipelineSetLayoutInfo}, CommandBuffer};

/// Descriptor set binding information to bind the actual resource to descriptor.
pub enum DescriptorSetBinding {
    Image(vk::DescriptorImageInfo),
    ImageArray(Vec<vk::DescriptorImageInfo>),
    Buffer(vk::DescriptorBufferInfo),

    #[cfg(feature = "gpu_ray_tracing")]
    RayTracingAccelStruct(vk::AccelerationStructureKHR),

    DynamicBuffer {
        buffer_info: vk::DescriptorBufferInfo,
        offset: u32,
    },
    DynamicStorageBuffer {
        buffer_info: vk::DescriptorBufferInfo,
        offset: u32,
    },
}

/// One Descriptor Set and its bindings information.
pub type PipelineSetBindings = BTreeMap<u32, DescriptorInfo>;
/// Descriptor Sets used in one pipeline.
pub type PipelineSetLayouts = BTreeMap<u32, PipelineSetBindings>;

/// Create descriptor set layouts from rspirv_reflect information.
/// This function assume that all the descriptor set layouts are used the same vk::ShaderStageFlags.
pub fn create_descriptor_set_layouts_with_unified_stage(
    device: &Device,
    set_layout_refl: &PipelineSetLayouts,
    stage_flag: vk::ShaderStageFlags,
) -> anyhow::Result<(Vec<vk::DescriptorSetLayout>, Vec<BTreeMap<u32, vk::DescriptorType>>)> {
    // calculate the descriptor set count
    let set_count = set_layout_refl.iter()
        .map(|(idx, _)| *idx + 1)
        .max()
        .unwrap_or(0_u32);

    let mut set_layouts: Vec<vk::DescriptorSetLayout> = Vec::with_capacity(set_count as usize);
    let mut set_layout_infos: Vec<BTreeMap<u32, vk::DescriptorType>> = Vec::with_capacity(set_count as usize);

    // for all the set, create its descriptor set layout
    for set_index in 0..set_count {
        // force overwrite stage flags
        let stage_flag = if set_index == 1 || set_index == 2 {
            vk::ShaderStageFlags::ALL
        } else {
            stage_flag
        };

        if let Some(set_bindings) = set_layout_refl.get(&set_index) {
            let (set_layout, set_layout_info) =
                create_descriptor_set_layout(&device, &set_bindings, stage_flag.clone())?;
    
            set_layouts.push(set_layout);
            set_layout_infos.push(set_layout_info);
        } else {
            // create a empty one
            let set_layout = unsafe {
                device.raw
                    .create_descriptor_set_layout(
                        &vk::DescriptorSetLayoutCreateInfo::builder().build(),
                        None,
                    )
                    .unwrap()
            };

            set_layouts.push(set_layout);
            set_layout_infos.push(Default::default());
        }
    }

    Ok((set_layouts, set_layout_infos))
}

/// Create a descriptor set layout for a single set.
/// This function assume that all descriptors are used in the same shader stage.
pub fn create_descriptor_set_layout(
    device: &Device,
    set_layout_refl: &PipelineSetBindings,
    stage_flag: vk::ShaderStageFlags,
) -> anyhow::Result<(vk::DescriptorSetLayout, BTreeMap<u32, vk::DescriptorType>)> {
    let mut bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::with_capacity(set_layout_refl.len());
    let mut binding_type_infos: BTreeMap<u32, vk::DescriptorType> = BTreeMap::new();
    // enable for all the resources.
    // if a resource in a descriptor had no memory access by shader, it can be invalid descriptor.
    let mut binding_flags: Vec<vk::DescriptorBindingFlags> = vec![vk::DescriptorBindingFlags::PARTIALLY_BOUND; set_layout_refl.len()];

    let mut set_layout_create_flags = vk::DescriptorSetLayoutCreateFlags::empty();

    let temp_sampler_refs = TempList::new();
    for (&binding_idx, binding_info) in set_layout_refl {
        let vk_descriptor_type = refl_descriptor_type_to_vk(binding_info.ty.clone(), &binding_info.name);

        match binding_info.ty {
            DescriptorType::UNIFORM_BUFFER
            | DescriptorType::UNIFORM_TEXEL_BUFFER
            | DescriptorType::STORAGE_IMAGE
            | DescriptorType::STORAGE_BUFFER
            | DescriptorType::STORAGE_BUFFER_DYNAMIC => {
                bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(binding_idx)
                        .descriptor_count(match binding_info.binding_count {
                            BindingCount::One => 1,
                            BindingCount::StaticSized(size) => size as u32,
                            BindingCount::Unbounded => unimplemented!()
                        })
                        .descriptor_type(vk_descriptor_type)
                        .stage_flags(stage_flag)
                        .build()
                );
            }
            DescriptorType::SAMPLED_IMAGE => {
                // it is a bindless sample image arrays
                if matches!(binding_info.binding_count, BindingCount::Unbounded) {
                    // enable all the bindless features for this binding
                    // now this binding can be updated as long as they are not dynamically used by any shader invocations.
                    // (dynamically used means any shader invocation executes an instruction that performs any memory access using this descriptor)
                    binding_flags[bindings.len()] =
                        vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                        | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING
                        | vk::DescriptorBindingFlags::PARTIALLY_BOUND
                        | vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;

                    set_layout_create_flags |= vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL;
                }

                let descriptor_count = match binding_info.binding_count {
                    BindingCount::One => 1,
                    BindingCount::StaticSized(size) => size as u32,
                    BindingCount::Unbounded => {
                        device.max_bindless_descriptor_count()
                    }
                };

                bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(binding_idx)
                        .descriptor_count(descriptor_count)
                        .descriptor_type(vk_descriptor_type)
                        .stage_flags(stage_flag)
                        .build()
                );
            }
            DescriptorType::SAMPLER => {
                assert!(binding_info.name.starts_with("sampler_"));
                let mut suffix = &binding_info.name["sampler_".len()..];

                let filter = match &suffix[..1] {
                    "l" => vk::Filter::LINEAR,
                    "n" => vk::Filter::NEAREST,
                    _ => panic!("Unsupported sampler filter mode: {}", &suffix[..1]),
                };
                suffix = &suffix[1..];
                
                let mipmap_mode = match &suffix[..1] {
                    "l" => vk::SamplerMipmapMode::LINEAR,
                    "n" => vk::SamplerMipmapMode::NEAREST,
                    _ => panic!("Unsupported sampler mipmap mode: {}", &suffix[..1]),
                };
                suffix = &suffix[1..];

                let address_mode = match &suffix[..] {
                    "ce" => vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    "cb" => vk::SamplerAddressMode::CLAMP_TO_BORDER,
                    "r" => vk::SamplerAddressMode::REPEAT,
                    "mr" => vk::SamplerAddressMode::MIRRORED_REPEAT,
                    "mce" => vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE,
                    _ => panic!("Unsupported sampler address mode: {}", &suffix[..]),
                };

                let sampler = device.get_immutable_sampler(SamplerDesc { filter, mipmap_mode, address_mode });
                bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::SAMPLER)
                        .stage_flags(stage_flag)
                        .binding(binding_idx)
                        .immutable_samplers(std::slice::from_ref(temp_sampler_refs.add(sampler)))
                        .build()
                );
            }
            #[cfg(feature = "gpu_ray_tracing")]
            DescriptorType::ACCELERATION_STRUCTURE_KHR => {
                bindings.push(vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding_idx)
                    .descriptor_count(1)
                    .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                    .stage_flags(stage_flag)
                    .build()
                );
            }
            _ => unimplemented!("Unimplemented descriptor type in {}", binding_idx)
        }

        binding_type_infos.insert(binding_idx, vk_descriptor_type);
    }

    let mut binding_flags_create_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::builder()
        .binding_flags(&binding_flags)
        .build();

    let set_layout = unsafe {
        device.raw
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder()
                    .push_next(&mut binding_flags_create_info)
                    .flags(set_layout_create_flags)
                    .bindings(&bindings)
                    .build(),
                None,
            )
            .expect("Failed to create vulkan descriptor set layout!")
    };

    Ok((set_layout, binding_type_infos))
}

/// Flatten all the descriptor set layouts in different shader stages into a single descriptor set layouts.
/// This is used to quickly gather all descriptors information in one pipelines.
/// Note: The flatten operation will not preserve the shader stage information.
pub(crate) fn flatten_all_stages_descriptor_set_layouts(
    stages: Vec<PipelineSetLayouts>,
) -> PipelineSetLayouts {
    let mut stages_iter = stages.into_iter();
    // if the stages is empty, then return a empty PipelineSetLayouts.
    let mut first = stages_iter.next().unwrap_or_default();

    // for the rest stages in the stages_iter, join them into the first one.
    for stage in stages_iter {
        add_descriptor_set_layouts(stage, &mut first);
    }

    first
}

pub(crate) fn add_descriptor_set_layouts(
    consumed: PipelineSetLayouts,
    add_to: &mut PipelineSetLayouts,
) {
    for set in consumed {
        // add set
        match add_to.entry(set.0) {
            // this set is already exists in add_to
            btree_map::Entry::Occupied(mut entry) => {
                let add_to_bindings = entry.get_mut(); 
                // add all the bindings to it
                for bindings in set.1 {
                    match add_to_bindings.entry(bindings.0) {
                        btree_map::Entry::Occupied(entry) => {
                            // exist, then it must be the same bindings! (But it is used in different shader stages)
                            assert_eq!(entry.get().name, bindings.1.name, "Descriptor Set Binding {}'s name in different pipeline stages is not the same: ({}, {})",
                                bindings.0, entry.get().name, bindings.1.name);
                            assert_eq!(entry.get().ty, bindings.1.ty, "Descriptor Set Binding {}'s type in different pipeline stages is not the same!",
                                bindings.0);
                        },
                        btree_map::Entry::Vacant(entry) => {
                            // not exist, add it.
                            entry.insert(bindings.1);
                        }
                    }
                }
            },
            btree_map::Entry::Vacant(entry) => {
                // not exist, add it.
                entry.insert(set.1);
            }
        }
    }
}

pub fn update_descriptor_set_buffer(
    device: &Device,
    dst_binding: u32,
    set: vk::DescriptorSet,
    ty: vk::DescriptorType,
    buffer: &Buffer, 
) {
    assert!(is_buffer_descriptor_type(&ty));

    let buffer_info = vk::DescriptorBufferInfo::builder()
        .buffer(buffer.raw)
        .range(vk::WHOLE_SIZE)
        .build();

        let write_descriptor_set = vk::WriteDescriptorSet::builder()
            .dst_set(set)
            .descriptor_type(ty)
            .dst_binding(dst_binding)
            .buffer_info(std::slice::from_ref(&buffer_info))
            .build();

        unsafe {
            device.raw
                .update_descriptor_sets(std::slice::from_ref(&write_descriptor_set), &[]);
        }
}

pub fn update_descriptor_set_image(
    device: &Device,
    dst_binding: u32,
    set: vk::DescriptorSet,
    ty: vk::DescriptorType,
    layout: vk::ImageLayout,
    image: &Image, 
) {
    assert!(is_image_descriptor_type(&ty));

    let image_info = vk::DescriptorImageInfo::builder()
        .image_layout(layout)
        .image_view(image.view(device, &ImageViewDesc::default()).unwrap())
        .build();

        let write_descriptor_set = vk::WriteDescriptorSet::builder()
            .dst_set(set)
            .descriptor_type(ty)
            .dst_binding(dst_binding)
            .image_info(std::slice::from_ref(&image_info))
            .build();

        unsafe {
            device.raw
                .update_descriptor_sets(std::slice::from_ref(&write_descriptor_set), &[]);
        }
}

/// Bind a descriptor set in place.
/// It will create a descriptor pool for this set and create the descriptor immediately.
/// The pool will be destroyed in next frame.
pub fn bind_descriptor_set_in_place(
    device: &Device,
    cmd: &CommandBuffer,
    set_idx: u32,
    pipeline: &CommonPipeline,
    bindings: &[DescriptorSetBinding],
) {
    let raw_device = &device.raw;
    let pipeline_info = &pipeline.pipeline_info;

    let pool = {
        let descriptor_pool_ci = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(1)
            .pool_sizes(&pipeline_info.descriptor_pool_sizes);

        unsafe { raw_device.create_descriptor_pool(&descriptor_pool_ci, None) }.unwrap()
    };

    // release in next frame
    device.defer_release(pool);

    // create descriptor set in place
    let descriptor_set = {
        let allocate_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(pool)
            .set_layouts(std::slice::from_ref(
                &pipeline_info.descriptor_set_layouts[set_idx as usize],
            ));

        unsafe { raw_device.allocate_descriptor_sets(&allocate_info) }.unwrap()[0]
    };

    let set_layout_info = if let Some(set_layout_info) = pipeline_info.set_layout_infos.get(set_idx as usize) {
        set_layout_info
    } else {
        panic!("Expect set {} but not found in pipeline shader!", set_idx)
    };

    let mut image_infos = TempList::new();
    let mut buffer_infos = TempList::new();
    #[cfg(feature = "gpu_ray_tracing")]
    let mut accel_infos = TempList::new();
    // TODO: use some memory arena to avoid frequently allocations and deallocations
    let mut dynamic_offsets: Vec<u32> = Vec::new();

    // update descriptor set and bind it
    #[cfg(not(feature = "gpu_ray_tracing"))]
    let writes = write_descriptor_set_bindings(
        bindings, set_layout_info, descriptor_set,
        &mut image_infos, &mut buffer_infos, &mut dynamic_offsets
    );

    #[cfg(feature = "gpu_ray_tracing")]
    let writes = write_descriptor_set_bindings(
        bindings, set_layout_info, descriptor_set,
        &mut image_infos, &mut buffer_infos, &mut accel_infos,
        &mut dynamic_offsets
    );

    unsafe {
        raw_device.update_descriptor_sets(&writes, &[]);

        raw_device.cmd_bind_descriptor_sets(
            cmd.raw,
            pipeline.pipeline_bind_point(),
            pipeline.pipeline_layout(), 
            set_idx, 
            &[descriptor_set], 
            dynamic_offsets.as_slice()
        );
    }
}

#[cfg(not(feature = "gpu_ray_tracing"))]
pub fn write_descriptor_set_bindings(
    bindings: &[DescriptorSetBinding], 
    set_layout_info: &PipelineSetLayoutInfo,
    set: vk::DescriptorSet,
    image_infos: &mut TempList<vk::DescriptorImageInfo>,
    buffer_infos: &mut TempList<vk::DescriptorBufferInfo>,
    dynamic_offsets: &mut Vec<u32>,
) -> Vec<vk::WriteDescriptorSet> {
    let writes = bindings.iter()
        .enumerate()
        // the binding must be defined in the pipeline shader
        .filter(|(binding_idx, _)| set_layout_info.contains_key(&(*binding_idx as u32)))
        .map(|(binding_idx, binding)| {
            let write = vk::WriteDescriptorSet::builder()
                .dst_set(set)
                .dst_binding(binding_idx as u32)
                .dst_array_element(0);

            match binding {
                DescriptorSetBinding::Image(image) => write
                    .descriptor_type(match image.image_layout {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                            vk::DescriptorType::SAMPLED_IMAGE
                        }
                        vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                        _ => unimplemented!(),
                    })
                    .image_info(std::slice::from_ref(image_infos.add(*image)))
                    .build(),
                DescriptorSetBinding::ImageArray(images) => {
                    assert!(!images.is_empty());

                    write.descriptor_type(match images[0].image_layout {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                            vk::DescriptorType::SAMPLED_IMAGE
                        }
                        vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                        _ => unimplemented!(),
                    })
                    .image_info(images.as_slice())
                    .build()
                }
                DescriptorSetBinding::Buffer(buffer) => write
                    // TODO: all is storage buffer??
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer)))
                    .build(),
                DescriptorSetBinding::DynamicBuffer { buffer_info, offset } => {
                    dynamic_offsets.push(*offset);
                    write.descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                        .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                        .build()
                }
                DescriptorSetBinding::DynamicStorageBuffer { buffer_info, offset } => {
                    dynamic_offsets.push(*offset);
                    write.descriptor_type(vk::DescriptorType::STORAGE_BUFFER_DYNAMIC)
                        .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                        .build()
                }
            }
        })
        .collect::<Vec<_>>();

    writes
}

#[cfg(feature = "gpu_ray_tracing")]
pub fn write_descriptor_set_bindings(
    bindings: &[DescriptorSetBinding], 
    set_layout_info: &PipelineSetLayoutInfo,
    set: vk::DescriptorSet,
    image_infos: &mut TempList<vk::DescriptorImageInfo>,
    buffer_infos: &mut TempList<vk::DescriptorBufferInfo>,
    accel_infos: &mut TempList<UnsafeCell<vk::WriteDescriptorSetAccelerationStructureKHR>>,
    dynamic_offsets: &mut Vec<u32>,
) -> Vec<vk::WriteDescriptorSet> {
    let writes = bindings.iter()
        .enumerate()
        // the binding must be defined in the pipeline shader
        .filter(|(binding_idx, _)| set_layout_info.contains_key(&(*binding_idx as u32)))
        .map(|(binding_idx, binding)| {
            let write = vk::WriteDescriptorSet::builder()
                .dst_set(set)
                .dst_binding(binding_idx as u32)
                .dst_array_element(0);

            match binding {
                DescriptorSetBinding::Image(image) => write
                    .descriptor_type(match image.image_layout {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                            vk::DescriptorType::SAMPLED_IMAGE
                        }
                        vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                        _ => unimplemented!(),
                    })
                    .image_info(std::slice::from_ref(image_infos.add(*image)))
                    .build(),
                DescriptorSetBinding::ImageArray(images) => {
                    assert!(!images.is_empty());

                    write.descriptor_type(match images[0].image_layout {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
                            vk::DescriptorType::SAMPLED_IMAGE
                        }
                        vk::ImageLayout::GENERAL => vk::DescriptorType::STORAGE_IMAGE,
                        _ => unimplemented!(),
                    })
                    .image_info(images.as_slice())
                    .build()
                }
                DescriptorSetBinding::Buffer(buffer) => write
                    // TODO: all is storage buffer??
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer)))
                    .build(),
                DescriptorSetBinding::DynamicBuffer { buffer_info, offset } => {
                    dynamic_offsets.push(*offset);
                    write.descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                        .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                        .build()
                }
                DescriptorSetBinding::DynamicStorageBuffer { buffer_info, offset } => {
                    dynamic_offsets.push(*offset);
                    write.descriptor_type(vk::DescriptorType::STORAGE_BUFFER_DYNAMIC)
                        .buffer_info(std::slice::from_ref(buffer_infos.add(*buffer_info)))
                        .build()
                }
                DescriptorSetBinding::RayTracingAccelStruct(accel_struct) => {
                    let mut write = write.descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                        .push_next(unsafe {
                            accel_infos.add(UnsafeCell::new(vk::WriteDescriptorSetAccelerationStructureKHR::builder()
                                .acceleration_structures(std::slice::from_ref(accel_struct))
                                .build(),
                            ))
                            .get().as_mut().unwrap()
                        })
                        .build();

                    write.descriptor_count = 1;
                    write
                }
            }
        })
        .collect::<Vec<_>>();

    writes
}

#[inline]
pub fn refl_descriptor_type_to_vk(ty: DescriptorType, name: &String) -> vk::DescriptorType {
    let is_dynamic = name.ends_with("_dyn");

    match ty {
        DescriptorType::SAMPLER => vk::DescriptorType::SAMPLER,
        DescriptorType::COMBINED_IMAGE_SAMPLER => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        DescriptorType::SAMPLED_IMAGE => vk::DescriptorType::SAMPLED_IMAGE,
        DescriptorType::STORAGE_IMAGE => vk::DescriptorType::STORAGE_IMAGE,
        DescriptorType::UNIFORM_TEXEL_BUFFER => vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
        DescriptorType::STORAGE_TEXEL_BUFFER => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
        DescriptorType::UNIFORM_BUFFER => {
            if is_dynamic {
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC
            } else {
                vk::DescriptorType::UNIFORM_BUFFER
            }
        },
        DescriptorType::STORAGE_BUFFER => {
            if is_dynamic {
                vk::DescriptorType::STORAGE_BUFFER_DYNAMIC
            } else {
                vk::DescriptorType::STORAGE_BUFFER
            }
        },
        DescriptorType::UNIFORM_BUFFER_DYNAMIC => vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
        DescriptorType::STORAGE_BUFFER_DYNAMIC => vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
        DescriptorType::INPUT_ATTACHMENT => vk::DescriptorType::INPUT_ATTACHMENT,
        
        DescriptorType::INLINE_UNIFORM_BLOCK_EXT => vk::DescriptorType::INLINE_UNIFORM_BLOCK_EXT,
        DescriptorType::ACCELERATION_STRUCTURE_KHR => vk::DescriptorType::ACCELERATION_STRUCTURE_KHR,
        DescriptorType::ACCELERATION_STRUCTURE_NV => vk::DescriptorType::ACCELERATION_STRUCTURE_NV,

        _ => unimplemented!(),
    }
}


#[inline]
fn is_buffer_descriptor_type(ty: &vk::DescriptorType) -> bool {
    match *ty {
        vk::DescriptorType::SAMPLER |
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER |
        vk::DescriptorType::SAMPLED_IMAGE |
        vk::DescriptorType::STORAGE_IMAGE |
        vk::DescriptorType::INPUT_ATTACHMENT => false,

        vk::DescriptorType::UNIFORM_TEXEL_BUFFER |
        vk::DescriptorType::STORAGE_TEXEL_BUFFER |
        vk::DescriptorType::UNIFORM_BUFFER |
        vk::DescriptorType::STORAGE_BUFFER |
        vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC |
        vk::DescriptorType::STORAGE_BUFFER_DYNAMIC => true,

        _ => false,
    }
}

#[inline]
fn is_image_descriptor_type(ty: &vk::DescriptorType) -> bool {
    match *ty {
        vk::DescriptorType::SAMPLER |
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER |
        vk::DescriptorType::SAMPLED_IMAGE |
        vk::DescriptorType::STORAGE_IMAGE => true,
        
        vk::DescriptorType::INPUT_ATTACHMENT |
        vk::DescriptorType::UNIFORM_TEXEL_BUFFER |
        vk::DescriptorType::STORAGE_TEXEL_BUFFER |
        vk::DescriptorType::UNIFORM_BUFFER |
        vk::DescriptorType::STORAGE_BUFFER |
        vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC |
        vk::DescriptorType::STORAGE_BUFFER_DYNAMIC => false,
        
        _ => false,
    }
}