use std::collections::HashMap;

use ash::vk::{self, DescriptorSetLayoutBinding};
use once_cell::sync::Lazy;
use raven_rhi::{Rhi, backend::descriptor};
use rspirv_reflect::{DescriptorType, DescriptorInfo, BindingCount};

fn get_engine_global_bindless_descriptor_layout() -> &'static HashMap<u32, DescriptorInfo> {
    static ENGINE_GLOBAL_BINDLESS_DESCRIPTOR_LAYOUT : Lazy<HashMap<u32, DescriptorInfo>> = Lazy::new(|| {
        [
            // mesh draw data (vertices and indices)
            (0, DescriptorInfo {
                ty: DescriptorType::STORAGE_BUFFER,
                binding_count: BindingCount::One,
                name: Default::default(),
            }),
            // mesh data (mesh draw data offsets)
            (1, rspirv_reflect::DescriptorInfo {
                ty: DescriptorType::STORAGE_BUFFER,
                binding_count: BindingCount::One,
                name: Default::default(),
            }),
        ]
        .into_iter()
        .collect::<HashMap<u32, DescriptorInfo>>()
    });

    &ENGINE_GLOBAL_BINDLESS_DESCRIPTOR_LAYOUT
}

pub fn create_engine_global_bindless_descriptor_set(rhi: &Rhi) -> vk::DescriptorSet {
    let raw_device = &rhi.device.raw;

    // if a resource in a descriptor had no memory access by shader, it can be invalid descriptor.
    let set_binding_flags = [
        vk::DescriptorBindingFlags::PARTIALLY_BOUND,
        vk::DescriptorBindingFlags::PARTIALLY_BOUND,
    ];

    let mut binding_flags_ci = vk::DescriptorSetLayoutBindingFlagsCreateInfo::builder()
        .binding_flags(&set_binding_flags)
        .build();

    let layout = get_engine_global_bindless_descriptor_layout();

    let mut bindings = Vec::new();
    for (binding_idx, info) in layout {
        let descriptor_count = match info.binding_count {
            BindingCount::One => 1,
            BindingCount::StaticSized(size) => size as u32,
            BindingCount::Unbounded => {
                rhi.device.max_bindless_descriptor_count()
            }
        };
        let ty = descriptor::refl_descriptor_type_to_vk(info.ty.clone(), &info.name);

        bindings.push(DescriptorSetLayoutBinding::builder()
            .binding(*binding_idx)
            .descriptor_count(descriptor_count)
            .descriptor_type(ty)
            .stage_flags(vk::ShaderStageFlags::ALL_GRAPHICS)
            .build()
        );
    }

    // TODO: add some helper functions in rhi
    let ci = vk::DescriptorSetLayoutCreateInfo::builder()
        .bindings(bindings.as_slice())
        //.flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
        .push_next(&mut binding_flags_ci)
        .build();

    let descriptor_set_layout = unsafe { 
        raw_device
            .create_descriptor_set_layout(&ci, None)
            .unwrap()
    };

    let descriptor_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 2,
        },
    ];

    // TODO: manually manage this pool
    let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
        .pool_sizes(&descriptor_sizes)
        //.flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
        .max_sets(1);

    let descriptor_pool = unsafe {
        raw_device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .unwrap()
    };

    // let variable_descriptor_count = rhi.device.max_bindless_descriptor_count();
    // let mut variable_descriptor_count_allocate_info =
    //     vk::DescriptorSetVariableDescriptorCountAllocateInfo::builder()
    //         .descriptor_counts(std::slice::from_ref(&variable_descriptor_count))
    //         .build();

    let descriptor_set = unsafe {
        raw_device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(std::slice::from_ref(&descriptor_set_layout))
                    // to have variable descriptor count in this set
                    //.push_next(&mut variable_descriptor_count_allocate_info)
                    .build(),
            )
            .unwrap()[0]
    };

    descriptor_set
}