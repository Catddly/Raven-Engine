use ash::vk;

use super::{physical_device::QueueFamily};

pub struct CommandBuffer {
    pub raw: vk::CommandBuffer,
    pub submit_done_fence: vk::Fence,
    
    _pool: vk::CommandPool,
}

impl CommandBuffer {
    pub fn new(device: &ash::Device, queue_family: &QueueFamily) -> Self {
        let fence_ci = vk::FenceCreateInfo::builder()
            // one the first frame, we assumed that it is submitted from '-1' frame, then we can go on submit the '0' frame
            .flags(vk::FenceCreateFlags::SIGNALED)
            .build();

        let fence = unsafe { device
            .create_fence(&fence_ci, None)
            .expect("Failed to create vulkan fence inside the command buffer!")
        };

        let pool_ci = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family.index)
            .build();

        let pool = unsafe { device
            .create_command_pool(&pool_ci, None)
            .expect("Failed to create vulkan command pool inside command buffer!")
        };

        let cb_alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .build();

        let command_buffers = unsafe {
            device
            .allocate_command_buffers(&cb_alloc_info)
            .expect("Failed to allocate vulkan command buffer from pool!")
        };

        Self {
            raw: command_buffers[0],
            _pool: pool,
            submit_done_fence: fence,
        }
    }
}