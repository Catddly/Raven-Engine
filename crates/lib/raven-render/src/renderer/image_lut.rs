use std::sync::Arc;

use raven_rg::{RenderGraphBuilder, RgHandle};
use raven_rhi::{
    Rhi,
    backend::{Image, AccessType}
};

pub trait ImageLutComputer: Send {
    fn create(&mut self, rhi: &Rhi) -> Image;
    fn compute(&mut self, rg: &mut RenderGraphBuilder, img: &mut RgHandle<Image>);
}

pub struct ImageLut {
    // this image will be destroyed in mesh_renderer
    image: Arc<Image>,
    computer: Box<dyn ImageLutComputer>,
    is_computed: bool,
}

impl ImageLut {
    pub fn new(rhi: &Rhi, computer: Box<dyn ImageLutComputer>) -> Self {
        let mut computer = computer;
        let image = computer.create(rhi);

        Self {
            image: Arc::new(image),
            computer,
            is_computed: false,
        }
    }

    pub fn compute_if_needed(&mut self, rg: &mut RenderGraphBuilder) {
        if !self.is_computed {
            let mut image = rg.import(self.image.clone(), AccessType::Nothing);
    
            self.computer.compute(rg, &mut image);
    
            rg.export(image, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
            
            self.is_computed = true;
        }
    }

    pub fn get_backing_image(&self) -> &Arc<Image> {
        // WARN: this will be invalid image until compute_if_needed() is at least called once
        &self.image
    }
}