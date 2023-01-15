use std::ops::Range;

use ash::vk;

use raven_math;

use crate::backend::{Device, Buffer, BufferDesc, RhiError};

pub trait CopyDataSource {
    fn as_bytes(&self) -> &[u8];
    fn alignment(&self) -> usize;
    fn is_empty(&self) -> bool;
}

impl<T: Copy> CopyDataSource for &[T] {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u8, 
                self.len() * std::mem::size_of::<T>()
            )
        }
    }

    fn alignment(&self) -> usize {
        std::mem::align_of::<T>()
    }

    fn is_empty(&self) -> bool {
        (self as &[T]).is_empty()
    }
}

impl<T: Copy> CopyDataSource for Vec<T> {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u8,
                self.len() * std::mem::size_of::<T>()
            )
        }
    }

    fn alignment(&self) -> usize {
        std::mem::align_of::<T>()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

pub struct CopyPrimitive<'a> {
    /// Copy data source
    source: Box<&'a dyn CopyDataSource>,
    /// Copy data offset
    offset: u32,
}

pub struct CopyEngine<'a> {
    copy_primitives: Vec<CopyPrimitive<'a>>,
    current_offset: u32,
}

impl<'a> CopyEngine<'a> {
    pub fn new() -> Self {
        Self {
            copy_primitives: Default::default(),
            current_offset: 0,
        }
    }

    pub fn current_offset(&self) -> u32 {
        self.current_offset
    }
    
    /// Copy the data from CopyDataSource and return an offset.
    pub fn copy(&mut self, source: &'a impl CopyDataSource) -> u32 {
        let alignment = source.alignment();
        // alignment must be the power of two or 1.
        assert_eq!(alignment.count_ones(), 1);

        let offset_beg = self.current_offset();
        assert!(offset_beg as usize % alignment == 0);
        let data_len = source.as_bytes().len();

        self.copy_primitives.push(CopyPrimitive {
            source: Box::new(source),
            offset: offset_beg,
        });
        self.current_offset = offset_beg + raven_math::min_value_align_to(data_len, alignment) as u32;

        offset_beg
    }

    pub fn upload(
        self, 
        device: &Device,
        dst_buffer: &Buffer,
        dst_offset: u32,
    ) -> anyhow::Result<(), RhiError> {
        // the copy data should not exceed the size of dst_buffer
        assert!(
            self.copy_primitives.iter()
                .map(|chunk| chunk.source.as_bytes().len())
                .sum::<usize>() + dst_offset as usize
            <= dst_buffer.desc.size
        );

        const STAGING_BUFFER_SIZE_BYTES: usize = 16 * 1024 * 1024;
        // TODO: use a common staging buffer for copy engine, and dispatch copy jobs to copy queue
        let mut staging_buffer = device.create_buffer(BufferDesc::new_cpu_to_gpu(
            STAGING_BUFFER_SIZE_BYTES, 
            vk::BufferUsageFlags::TRANSFER_SRC), 
            "copy engine staging buffer"
        )?;

        struct UploadChunk {
            source_idx: usize,
            source_range: Range<usize>,
        }

        let chunks = self.copy_primitives.iter()
            .enumerate()
            .flat_map(|(source_idx, prim)| {
                let bytes_len = prim.source.as_bytes().len();
                //glog::debug!("try to upload {:?}", prim.source.as_bytes());
                
                // take ceil of bytes_len
                let num_chunk = (bytes_len + STAGING_BUFFER_SIZE_BYTES - 1) / STAGING_BUFFER_SIZE_BYTES;

                (0..num_chunk).map(move |idx| UploadChunk {
                    source_idx,
                    source_range: (idx * STAGING_BUFFER_SIZE_BYTES)..((idx + 1) * STAGING_BUFFER_SIZE_BYTES).min(bytes_len)
                })
            })
            .collect::<Vec<_>>();

        for UploadChunk {
            source_idx,
            source_range,
        } in chunks {
            let prim = &self.copy_primitives[source_idx];
            // memcpy to staging buffer
            staging_buffer.allocation.mapped_slice_mut().unwrap()[0..(source_range.end - source_range.start)]
                .copy_from_slice(&prim.source.as_bytes()[source_range.start..source_range.end]);

            // TODO: this is slow if we do upload every frame
            device.with_setup_commands(|cb| {
                unsafe {
                    device.raw.cmd_copy_buffer(
                    cb,
                    staging_buffer.raw, 
                    dst_buffer.raw, 
                    &[vk::BufferCopy::builder()
                        .src_offset(0)
                        .dst_offset((dst_offset + prim.offset + source_range.start as u32) as u64)
                        .size((source_range.end - source_range.start) as u64)
                        .build()
                    ]);
                }
            })?;
        }

        device.destroy_buffer(staging_buffer);

        Ok(())
    }
}