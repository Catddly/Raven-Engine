use std::collections::{btree_map, BTreeMap};

use rspirv_reflect::{DescriptorInfo, DescriptorType, BindingCount};
use ash::vk;

use crate::backend::ImageViewDesc;

use super::{Device, Buffer, Image};

/// Descriptor set binding information to bind the actual resource to descriptor.
pub enum DescriptorSetBinding {
    Image(vk::DescriptorImageInfo),
    ImageArray(Vec<vk::DescriptorImageInfo>),
    Buffer(vk::DescriptorBufferInfo),

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
        // override set 0's stage flag
        let stage_flag = if set_index == 2 {
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
            },
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
            },
            // TODO: add immutable sampler
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