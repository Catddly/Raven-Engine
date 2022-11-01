use ash::vk;
use rspirv_reflect::{DescriptorInfo, DescriptorType, BindingCount};
use once_cell::sync::Lazy;

use raven_rhi::{Rhi, backend::{descriptor::{self, PipelineSetBindings}}, dynamic_buffer::DynamicBuffer};

// to be used in set 2
fn get_engine_global_constant_descriptor_layout() -> &'static PipelineSetBindings {
    static ENGINE_GLOBAL_CONSTANTS_DESCRIPTOR_LAYOUT : Lazy<PipelineSetBindings> = Lazy::new(|| {
        [
            // frame constants
            (0, DescriptorInfo {
                ty: DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                binding_count: BindingCount::One,
                name: Default::default(),
            }),
        ]
        .into_iter()
        .collect::<PipelineSetBindings>()
    });

    &ENGINE_GLOBAL_CONSTANTS_DESCRIPTOR_LAYOUT
}

pub fn create_engine_global_constants_descriptor_set(rhi: &Rhi, buffer: &DynamicBuffer) -> vk::DescriptorSet {
    let raw_device = &rhi.device.raw;

    // if a resource in a descriptor had no memory access by shader, it can be invalid descriptor.
    let set_binding_flags = [
        vk::DescriptorBindingFlags::PARTIALLY_BOUND,
    ];

    let mut binding_flags_ci = vk::DescriptorSetLayoutBindingFlagsCreateInfo::builder()
        .binding_flags(&set_binding_flags)
        .build();

    let layout = get_engine_global_constant_descriptor_layout();

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

        bindings.push(vk::DescriptorSetLayoutBinding::builder()
            .binding(*binding_idx)
            .descriptor_count(descriptor_count)
            .descriptor_type(ty)
            .stage_flags(vk::ShaderStageFlags::ALL)
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
            ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
            descriptor_count: 1,
        },
    ];

    // TODO: manually manage this pool
    let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
        .pool_sizes(&descriptor_sizes)
        .max_sets(1);

    let descriptor_pool = unsafe {
        raw_device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .unwrap()
    };

    let descriptor_set = unsafe {
        raw_device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(std::slice::from_ref(&descriptor_set_layout))
                    .build(),
            )
            .unwrap()[0]
    };

    // update descriptor set
    let uniform_buffer_info = vk::DescriptorBufferInfo::builder()
        .buffer(buffer.buffer.raw)
        .range(buffer.max_uniform_buffer_range() as u64)
        .build();

    let descriptor_set_writes = [
        // frame constants
        vk::WriteDescriptorSet::builder()
            .dst_binding(0)
            .dst_set(descriptor_set)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .buffer_info(std::slice::from_ref(&uniform_buffer_info))
            .build(),
    ];

    unsafe { raw_device.update_descriptor_sets(&descriptor_set_writes, &[]) };

    descriptor_set
}