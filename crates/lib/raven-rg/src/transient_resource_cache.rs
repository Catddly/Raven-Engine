use std::collections::HashMap;

use raven_rhi::backend::{Device, BufferDesc, ImageDesc, Image, Buffer};

/// Transient resource cache to cache resources created by the render graph.
/// 
/// # Note
/// 
/// Actually, some resources will be used permanently in the render graph, transient here means that
/// the resources are not used by all the passes in the render graph. (i.e. they only used by some passes)
pub struct TransientResourceCache {
    images: HashMap<ImageDesc, Vec<Image>>,
    buffers: HashMap<BufferDesc, Vec<Buffer>>,
}

impl TransientResourceCache {
    pub fn new() -> Self {
        Self {
            images: Default::default(),
            buffers: Default::default(),
        }
    }

    pub fn get_buffer(&mut self, desc: &BufferDesc) -> Option<Buffer> {
        if let Some(vec) = self.buffers.get_mut(desc) {
            vec.pop()
        } else {
            None
        }
    }

    pub fn get_image(&mut self, desc: &ImageDesc) -> Option<Image> {
        if let Some(vec) = self.images.get_mut(desc) {
            vec.pop()
        } else {
            None
        }
    }

    pub fn store_image(&mut self, image: Image) {
        if let Some(vec) = self.images.get_mut(&image.desc) {
            vec.push(image);
        } else {
            self.images.insert(image.desc, vec![image]);
        }
    }

    pub fn store_buffer(&mut self, buffer: Buffer) {
        if let Some(vec) = self.buffers.get_mut(&buffer.desc) {
            vec.push(buffer);
        } else {
            self.buffers.insert(buffer.desc, vec![buffer]);
        }
    }

    pub fn clean(self, device: &Device) {
        for (_, images) in self.images {
            for image in images {
                device.destroy_image(image);
            }
        }

        for (_, buffers) in self.buffers {
            for buffer in buffers {
                device.destroy_buffer(buffer);
            }
        }
    }
}