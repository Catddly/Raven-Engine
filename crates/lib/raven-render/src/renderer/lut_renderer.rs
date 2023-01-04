use ash::vk;

use raven_core::utility;
use raven_rg::{RgHandle, RenderGraphBuilder, IntoPipelineDescriptorBindings, RenderGraphPassBindable};
use raven_rhi::{
    Rhi,
    backend::{Image, ImageDesc, AccessType}
};

use super::image_lut::ImageLutComputer;

pub const BRDF_LUT_IMAGE_RESOLUTION: u32 = 512;

pub struct BrdfLutComputer;

impl ImageLutComputer for BrdfLutComputer {
    fn create(&mut self, rhi: &Rhi) -> Image {
        let brdf = rhi.device.create_image(ImageDesc::new_2d([BRDF_LUT_IMAGE_RESOLUTION, BRDF_LUT_IMAGE_RESOLUTION], vk::Format::R16G16_SFLOAT)
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create brdf lut!");

        brdf
    }

    fn compute(&mut self, rg: &mut RenderGraphBuilder, img: &mut RgHandle<Image>) {
        let mut pass = rg.add_pass("brdf lut");
        let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_specular_brdf.hlsl");

        let brdf_ref = pass.write(img, AccessType::ComputeShaderWrite);

        pass.render(move |ctx| {
            let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                .descriptor_set(0, &[
                    brdf_ref.bind(),
                ])
            )?;

            let push_constants = [BRDF_LUT_IMAGE_RESOLUTION, BRDF_LUT_IMAGE_RESOLUTION];
            bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, utility::as_byte_slice_val(&push_constants));

            bound_pipeline.dispatch([BRDF_LUT_IMAGE_RESOLUTION, BRDF_LUT_IMAGE_RESOLUTION, 1]);

            Ok(())
        });
    }
}