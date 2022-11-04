use std::sync::Arc;

use ash::vk;

use raven_core::utility::as_byte_slice_values;
use raven_rg::{RenderGraphBuilder, RgHandle, RenderGraphPassBindable, IntoPipelineDescriptorBindings};
use raven_rhi::{backend::{Image, AccessType, ImageDesc}, Rhi};

pub struct IblRenderer {
    convolved_cubemap: Arc<Image>,
    prefilter_cubemap: Arc<Image>,
    brdf_lut: Arc<Image>,
    need_convolve: bool,
}

impl IblRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let convolved = rhi.device.create_image(ImageDesc::new_cube(64, vk::Format::R8G8B8A8_UNORM)
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create convolved cubamap!");

        let prefilter = rhi.device.create_image(ImageDesc::new_cube(512, vk::Format::R16G16B16A16_SFLOAT)
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create prefilter cubamap!");

        let brdf = rhi.device.create_image(ImageDesc::new_2d([512, 512], vk::Format::R16G16_SFLOAT)
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create brdf lut!");

        Self {
            convolved_cubemap: Arc::new(convolved),
            prefilter_cubemap: Arc::new(prefilter),
            brdf_lut: Arc::new(brdf),
            need_convolve: true,
        }
    }

    pub fn convolve_if_needed(&mut self, rg: &mut RenderGraphBuilder, cubemap: &RgHandle<Image>) -> (RgHandle<Image>, RgHandle<Image>, RgHandle<Image>) {
        let (mut convolve_cubemap, mut prefilter_cubemap, mut brdf_lut) = if self.need_convolve {
            (
                rg.import(self.convolved_cubemap.clone(), AccessType::Nothing),
                rg.import(self.prefilter_cubemap.clone(), AccessType::Nothing),
                rg.import(self.brdf_lut.clone(), AccessType::Nothing),
            )
        } else {
            (
                rg.import(self.convolved_cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer),
                rg.import(self.prefilter_cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer),
                rg.import(self.brdf_lut.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer),
            )
        };

        if self.need_convolve {
            {
                let mut pass = rg.add_pass("convolve cubemap");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/convolve_cubemap.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let convolve_cubemap_ref = pass.write(&mut convolve_cubemap, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let mut convolve_cubemap_binding = convolve_cubemap_ref.bind();
                    convolve_cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);
    
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            cubemap_ref.bind(),
                            convolve_cubemap_binding
                        ])
                    )?;
    
                    let push_constants = [64, 64];
                    bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
    
                    bound_pipeline.dispatch([64, 64, 6]);
    
                    Ok(())
                });
            }

            {
                let mut pass = rg.add_pass("prefilter cubemap");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/prefilter_cubemap.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let prefilter_cubemap_ref = pass.write(&mut prefilter_cubemap, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let mut prefilter_cubemap_binding = prefilter_cubemap_ref.bind();
                    prefilter_cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);
    
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            cubemap_ref.bind(),
                            prefilter_cubemap_binding
                        ])
                    )?;
    
                    let push_constants = [512, 512];
                    bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
    
                    bound_pipeline.dispatch([512, 512, 6]);
    
                    Ok(())
                });
            }

            {
                let mut pass = rg.add_pass("brdf lut");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/generate_brdf_lut.hlsl");
    
                let brdf_ref = pass.write(&mut brdf_lut, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            brdf_ref.bind(),
                        ])
                    )?;
    
                    let push_constants = [512, 512];
                    bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
    
                    bound_pipeline.dispatch([512, 512, 1]);
    
                    Ok(())
                });
            }

            self.need_convolve = false;
        }

        (convolve_cubemap, prefilter_cubemap, brdf_lut)
    }

    pub fn clean(self, rhi: &Rhi) {
        let convolved_cubemap = Arc::try_unwrap(self.convolved_cubemap)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));
        let prefilter_cubemap = Arc::try_unwrap(self.prefilter_cubemap)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));
        let brdf_lut = Arc::try_unwrap(self.brdf_lut)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));

        rhi.device.destroy_image(convolved_cubemap);
        rhi.device.destroy_image(prefilter_cubemap);
        rhi.device.destroy_image(brdf_lut);
    }
}