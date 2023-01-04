use ash::vk;

use raven_core::{utility::as_byte_slice, math};

use crate::{backend::{Buffer, BufferDesc, Device}, Rhi};

pub const MAX_DYNAMIC_BUFFER_SIZE_BYTES: usize = 1024 * 1024 * 16;
pub const MAX_DYNAMIC_BUFFER_FRAME_COUNT: usize = 2;

pub struct DynamicBuffer {
    pub buffer: Buffer,
    /// Offset should always be aligned to alignment
    prev_offset_bytes: u32, // cached previous push data offset to have the ability to reuse some buffer data
    current_offset_bytes: u32,
    current_frame: u32,

    alignment: u32,
    max_uniform_buffer_range: u32,
    max_storage_buffer_range: u32,
}

impl DynamicBuffer {
    pub fn new(rhi: &Rhi) -> Self {
        // total size of the buffer should be MAX_DYNAMIC_BUFFER_FRAME_COUNT * MAX_DYNAMIC_BUFFER_SIZE_BYTES;
        let buffer = rhi.device.create_buffer(
            BufferDesc::new_cpu_to_gpu(
                MAX_DYNAMIC_BUFFER_FRAME_COUNT * MAX_DYNAMIC_BUFFER_SIZE_BYTES, 
                vk::BufferUsageFlags::UNIFORM_BUFFER |
                vk::BufferUsageFlags::STORAGE_BUFFER |
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            ),
            "dynamic buffer"
        ).expect("Failed to create dynamic buffer!");

        let limits = &rhi.device.physical_device.properties.limits;
        let alignment = limits.min_uniform_buffer_offset_alignment.max(limits.min_storage_buffer_offset_alignment);

        Self {
            buffer,
            prev_offset_bytes: 0,
            current_offset_bytes: 0,
            current_frame: 0,

            alignment: alignment as _,
            max_uniform_buffer_range: limits.max_uniform_buffer_range,
            max_storage_buffer_range: limits.max_storage_buffer_range,
        }
    }

    #[inline]
    pub fn max_uniform_buffer_range(&self) -> u32 {
        self.max_uniform_buffer_range
    }
    
    #[inline]
    pub fn max_storage_buffer_range(&self) -> u32 {
        self.max_storage_buffer_range
    }

    pub fn advance_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % MAX_DYNAMIC_BUFFER_FRAME_COUNT as u32;
        // reset next frame's buffer data
        self.prev_offset_bytes = 0;
        self.current_offset_bytes = 0;
    }

    #[inline]
    fn current_offset(&self) -> u32 {
        (self.current_frame * MAX_DYNAMIC_BUFFER_SIZE_BYTES as u32) + self.current_offset_bytes
    }

    #[inline]
    pub fn current_device_address(&self, device: &Device) -> vk::DeviceAddress {
        self.buffer.device_address(device) + (self.current_offset() as vk::DeviceAddress)
    }

    #[inline]
    pub fn previous_pushed_data_offset(&self) -> u32 {
        self.prev_offset_bytes
    }

    /// Push a value into GPU dynamic buffer, returning the offset in the buffer.
    pub fn push<T: Copy>(&mut self, value: &T) -> u32 {
        let t_size = std::mem::size_of::<T>();
        // can not exceed the max buffer size
        assert!(self.current_offset_bytes as usize + t_size <= MAX_DYNAMIC_BUFFER_SIZE_BYTES);

        let curr_offset = self.current_offset() as usize;
        self.prev_offset_bytes = curr_offset as u32;

        let copy_slice = &mut self.buffer.allocation.mapped_slice_mut().unwrap()[curr_offset..curr_offset + t_size];
        copy_slice.copy_from_slice(as_byte_slice(value));

        self.current_offset_bytes += math::min_value_align_to(t_size, self.alignment as usize) as u32;

        curr_offset as _
    }

    pub fn push_from_iter<T: Copy, Iter: Iterator<Item = T>>(&mut self, iter: Iter) -> u32 {
        let t_size = std::mem::size_of::<T>();
        let t_align = std::mem::align_of::<T>();
        // can not exceed the max buffer size
        assert!(self.current_offset_bytes as usize + t_size <= MAX_DYNAMIC_BUFFER_SIZE_BYTES);
        // alignment must be consistent
        assert!(self.alignment as usize % t_align == 0);

        let curr_offset = self.current_offset() as usize;
        self.prev_offset_bytes = curr_offset as u32;

        // current offset must be aligned to t_align
        assert!(curr_offset % t_align == 0);
        
        let mut offset = curr_offset;

        // TODO: optimize: should be faster to copy once, instead of copy n times
        for v in iter {
            let copy_slice = &mut self.buffer.allocation.mapped_slice_mut().unwrap()[offset..offset + t_size];
            copy_slice.copy_from_slice(as_byte_slice(&v));
            // array elements should be aligned
            offset += math::min_value_align_to(t_size, t_align);
        }

        self.current_offset_bytes += math::min_value_align_to(offset - curr_offset, self.alignment as usize) as u32;

        curr_offset as _
    }

    pub fn clean(self, device: &Device) {
        device.destroy_buffer(self.buffer)
    }
}