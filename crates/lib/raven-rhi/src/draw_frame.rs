use parking_lot::Mutex;
use ash::vk;

use crate::backend::{CommandBuffer, physical_device::QueueFamily, Buffer, Device};

pub trait DeferReleasableResource {
    fn enqueue(self, queue: &mut DeferReleaseQueue);
}

impl DeferReleasableResource for vk::DescriptorPool {
    fn enqueue(self, queue: &mut DeferReleaseQueue) {
        queue.descriptor_pools.push(self);
    }
}

impl DeferReleasableResource for Buffer {
    fn enqueue(self, queue: &mut DeferReleaseQueue) {
        queue.buffers.push(self);
    }
}

pub struct DeferReleaseQueue {
    descriptor_pools: Vec<vk::DescriptorPool>,
    buffers: Vec<Buffer>,
}

impl DeferReleaseQueue {
    fn new() -> Self {
        Self {
            descriptor_pools: Default::default(),
            buffers: Default::default(),
        }
    }
}

pub struct DrawFrame {
    pub swapchain_acquired_semaphore: vk::Semaphore,
    pub render_complete_semaphore: vk::Semaphore,

    pub main_command_buffer: CommandBuffer,
    pub present_command_buffer: CommandBuffer,

    pub defer_release_resources: Mutex<DeferReleaseQueue>,
}

impl DrawFrame {
    pub fn new(
        device: &ash::Device,
        queue_family: &QueueFamily,
    ) -> Self {
        let swapchain_acquired_semaphore = unsafe { device
            .create_semaphore(&vk::SemaphoreCreateInfo::builder().build(), None)
            .unwrap()
        };
        let render_complete_semaphore = unsafe { device
            .create_semaphore(&vk::SemaphoreCreateInfo::builder().build(), None)
            .unwrap()
        };

        Self {
            swapchain_acquired_semaphore,
            render_complete_semaphore,

            main_command_buffer: CommandBuffer::new(&device, &queue_family),
            present_command_buffer:CommandBuffer::new(&device, &queue_family),

            defer_release_resources: Mutex::new(DeferReleaseQueue::new()),
        }
    }

    pub fn release_stale_render_resources(&self, device: &Device) {
        let mut defer_release_resources = self.defer_release_resources.lock();

        for pool in defer_release_resources.descriptor_pools.drain(..) {
            unsafe { device.raw.destroy_descriptor_pool(pool, None); }
        }

        for buffer in defer_release_resources.buffers.drain(..) {
            device.destroy_buffer(buffer);
        }
    }
}