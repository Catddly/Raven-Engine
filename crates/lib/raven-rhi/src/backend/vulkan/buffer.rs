use std::hash::Hash;

use ash::vk;

use super::allocator::{Allocator, MemoryLocation, AllocationCreateDesc, Allocation, self};
use super::{Device, error};
use super::RHIError;

pub struct Buffer {
    pub raw: vk::Buffer,
    pub desc: BufferDesc,
    pub allocation: Allocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    pub size: usize,
    pub alignment: Option<usize>,
    pub usage: vk::BufferUsageFlags,
    pub memory_location: MemoryLocation,
}

impl BufferDesc {
    pub fn new_gpu_to_cpu(size: usize, usage: vk::BufferUsageFlags) -> Self {
        BufferDesc {
            size,
            usage,
            memory_location: MemoryLocation::GpuToCpu,
            alignment: None,
        }
    }

    pub fn new_gpu_only(size: usize, usage: vk::BufferUsageFlags) -> Self {
        BufferDesc {
            size,
            usage,
            memory_location: MemoryLocation::GpuOnly,
            alignment: None,
        }
    }

    pub fn new_cpu_to_gpu(size: usize, usage: vk::BufferUsageFlags) -> Self {
        BufferDesc {
            size,
            usage,
            memory_location: MemoryLocation::CpuToGpu,
            alignment: None,
        }
    }
}

// implement buffer associated function for device
impl Device {
    pub fn create_buffer(
        &self,
        desc: BufferDesc,
        name: &str,
    ) -> anyhow::Result<Buffer, RHIError> {
        let buffer = Self::create_buffer_internal(&self.raw, &mut self.global_allocator.lock(), desc, &name)?;

        Ok(buffer)
    }

    pub fn destroy_buffer(
        &self,
        buffer: Buffer
    ) {
        unsafe {
            self.raw.destroy_buffer(buffer.raw, None)
        }
        self.global_allocator
            .lock()
            .free(buffer.allocation)
            .expect("Failed to free memory of vulkan buffer!");
    }

    pub(crate) fn create_buffer_internal(
        device: &ash::Device,
        allocator: &mut Allocator,
        desc: BufferDesc,
        name: &str // name in here is just for debug purpose
    ) -> anyhow::Result<Buffer, error::RHIError> {
        let create_info = vk::BufferCreateInfo {
            size: desc.size as u64,
            usage: desc.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let buffer = unsafe { device.create_buffer(&create_info, None) }
            .expect("Failed to create vulkan buffer!");
        // get memory requirement
        let mut requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        if let Some(alignment) = desc.alignment {
            requirements.alignment = requirements.alignment.max(alignment as u64);
        }

        let allocation = allocator
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location: allocator::to_inner_memory_location(&desc.memory_location),
                linear: true, // buffer is always consecutive in memory
            })
            .map_err(move |err| RHIError::AllocationFailure { 
                name: name.to_owned(), 
                error: err 
            })?;

        // bind memory to the buffer
        // TODO: can have multiple buffer binds to same memory.
        unsafe {
            device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .expect("Failed to bind vulkan buffer memory!");
        }

        Ok(Buffer {
            raw: buffer,
            desc,
            allocation,
        })
    }
}