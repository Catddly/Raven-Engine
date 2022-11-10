use std::sync::Arc;

use ash::vk;

use raven_core::{utility::as_byte_slice_values, math::max_mipmap_level};
use raven_rg::{RenderGraphBuilder, RgHandle, RenderGraphPassBindable, IntoPipelineDescriptorBindings};
use raven_rhi::{backend::{Image, AccessType, ImageDesc}, Rhi};

const CONVOLVED_CUBEMAP_RESOLUTION: usize = 64;
const PREFILTER_CUBEMAP_RESOLUTION: usize = 512;

pub struct IblRenderer {
    convolved_cubemap: Arc<Image>,
    prefilter_cubemap: Arc<Image>,
    brdf_lut: Arc<Image>,
    need_convolve: bool,
}

impl IblRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let convolved = rhi.device.create_image(ImageDesc::new_cube(CONVOLVED_CUBEMAP_RESOLUTION as _, vk::Format::R16G16B16A16_SFLOAT)
            .mipmap_level(max_mipmap_level(CONVOLVED_CUBEMAP_RESOLUTION as _, CONVOLVED_CUBEMAP_RESOLUTION as _))
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create convolved cubamap!");

        let prefilter = rhi.device.create_image(ImageDesc::new_cube(PREFILTER_CUBEMAP_RESOLUTION as _, vk::Format::R16G16B16A16_SFLOAT)
            .mipmap_level(max_mipmap_level(PREFILTER_CUBEMAP_RESOLUTION as _, PREFILTER_CUBEMAP_RESOLUTION as _))
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
                let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_diffuse.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let convolve_cubemap_ref = pass.write(&mut convolve_cubemap, AccessType::ComputeShaderWrite);
    
                let cubemap_extent = cubemap.desc().extent;
                pass.render(move |ctx| {
                    let mip_level = max_mipmap_level(CONVOLVED_CUBEMAP_RESOLUTION as _, CONVOLVED_CUBEMAP_RESOLUTION as _);
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings())?;
             
                    for level in 0..mip_level {
                        let mut cubemap_binding = cubemap_ref.bind();
                        cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);

                        let mut convolve_cubemap_binding = convolve_cubemap_ref.bind();
                        convolve_cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);
                        convolve_cubemap_binding.with_base_mipmap_level(level);

                        bound_pipeline.rebind(0, &[
                            cubemap_binding, convolve_cubemap_binding
                        ])?;

                        let convolved_res = (CONVOLVED_CUBEMAP_RESOLUTION >> level).max(1) as u32;
    
                        let push_constants = [cubemap_extent[0], convolved_res];
                        bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
        
                        bound_pipeline.dispatch([convolved_res.max(8), convolved_res.max(8), 6]);
                    }
                    
                    Ok(())
                });
            }

            {
                let mut pass = rg.add_pass("prefilter cubemap");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_specular_light.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let prefilter_cubemap_ref = pass.write(&mut prefilter_cubemap, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let mip_level = max_mipmap_level(PREFILTER_CUBEMAP_RESOLUTION as _, PREFILTER_CUBEMAP_RESOLUTION as _);
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings())?;
    
                    for level in 0..mip_level {
                        let mut prefilter_cubemap_binding = prefilter_cubemap_ref.bind();
                        prefilter_cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);
                        prefilter_cubemap_binding.with_base_mipmap_level(level);

                        bound_pipeline.rebind(0, &[
                            cubemap_ref.bind(), prefilter_cubemap_binding
                        ])?;

                        let convolved_res = (PREFILTER_CUBEMAP_RESOLUTION >> level).max(1) as u32;
    
                        // TODO: bug found! When using three u32 value, the cmd_dispatch will crash
                        let push_constants = [PREFILTER_CUBEMAP_RESOLUTION as u32, convolved_res];
                        bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
        
                        bound_pipeline.dispatch([convolved_res.max(8), convolved_res.max(8), 6]);
                    }

                    Ok(())
                });
            }

            {
                let mut pass = rg.add_pass("brdf lut");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_specular_brdf.hlsl");
    
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