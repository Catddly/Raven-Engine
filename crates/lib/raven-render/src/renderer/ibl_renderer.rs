use std::sync::Arc;

use ash::vk;

use raven_core::{utility::as_byte_slice_val, math::{self, SHBasis9}};
use raven_rg::{RenderGraphBuilder, RgHandle, RenderGraphPassBindable, IntoPipelineDescriptorBindings};
use raven_rhi::{backend::{Image, AccessType, ImageDesc, Buffer, BufferDesc}, Rhi, copy_engine::CopyEngine};

const PREFILTER_CUBEMAP_RESOLUTION: usize = 512;

pub struct IblRenderer {
    sh_buffer: Arc<Buffer>,

    //convolved_cubemap: Arc<Image>,
    prefilter_cubemap: Arc<Image>,

    ibl_resources_prepared: bool,
}

impl IblRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        // let convolved = rhi.device.create_image(ImageDesc::new_cube(CONVOLVED_CUBEMAP_RESOLUTION as _, vk::Format::R16G16B16A16_SFLOAT)
        //     .mipmap_level(max_mipmap_level(CONVOLVED_CUBEMAP_RESOLUTION as _, CONVOLVED_CUBEMAP_RESOLUTION as _))
        //     .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
        //     .expect("Failed to create convolved cubamap!");

        let prefilter = rhi.device.create_image(ImageDesc::new_cube(PREFILTER_CUBEMAP_RESOLUTION as _, vk::Format::R16G16B16A16_SFLOAT)
            .mipmap_level(math::max_mipmap_level_2d(PREFILTER_CUBEMAP_RESOLUTION as _, PREFILTER_CUBEMAP_RESOLUTION as _))
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create prefilter cubamap!");

        let sh_buffer = rhi.device.create_buffer(
            BufferDesc::new_gpu_only(3 * 9 * std::mem::size_of::<f32>(),
             vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::UNIFORM_BUFFER | vk::BufferUsageFlags::TRANSFER_DST
            ), "ibl sh buffer"
        ).expect("Failed to create ibl sh buffer!");

        Self {
            sh_buffer: Arc::new(sh_buffer),

            prefilter_cubemap: Arc::new(prefilter),

            ibl_resources_prepared: true,
        }
    }

    pub fn update_sh(&mut self, rhi: &Rhi, basis: [SHBasis9; 3]) {
        let values = [
            basis[0].to_f32_array(),
            basis[1].to_f32_array(),
            basis[2].to_f32_array(),
        ];
        let values = values.concat();
        
        let mut copy_engine = CopyEngine::new();
        copy_engine.copy(&values);
        copy_engine.upload(&rhi.device, &self.sh_buffer, 0).expect("Failed to upload sh buffer data!");
    }

    pub fn prepare_ibl_if_needed(&mut self, rg: &mut RenderGraphBuilder, cubemap: &RgHandle<Image>) -> (RgHandle<Buffer>, RgHandle<Image>) {
        let (mut sh_buffer, mut prefilter_cubemap) = if self.ibl_resources_prepared {
            (
                rg.import(self.sh_buffer.clone(), AccessType::Nothing),
                rg.import(self.prefilter_cubemap.clone(), AccessType::Nothing),
            )
        } else {
            (
                rg.import(self.sh_buffer.clone(), AccessType::AnyShaderReadUniformBuffer),
                rg.import(self.prefilter_cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer),
            )
        };

        if self.ibl_resources_prepared {
            {
                let mut pass = rg.add_pass("convolve cubemap");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_diffuse_sh.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let sh_buffer_ref = pass.write(&mut sh_buffer, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let mut cubemap_binding = cubemap_ref.bind();
                    cubemap_binding.with_image_view(vk::ImageViewType::TYPE_2D_ARRAY);

                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            cubemap_binding,
                            sh_buffer_ref.bind(),
                        ])
                    )?;

                    bound_pipeline.dispatch([8, 8, 1]);
                    
                    Ok(())
                });
            }

            {
                let mut pass = rg.add_pass("prefilter cubemap");
                let pipeline = pass.register_compute_pipeline("pbr/ibl/ibl_specular_light.hlsl");
    
                let cubemap_ref = pass.read(cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer);
                let prefilter_cubemap_ref = pass.write(&mut prefilter_cubemap, AccessType::ComputeShaderWrite);
    
                pass.render(move |ctx| {
                    let mip_level = math::max_mipmap_level_2d(PREFILTER_CUBEMAP_RESOLUTION as _, PREFILTER_CUBEMAP_RESOLUTION as _);
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
                        bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_val(&push_constants));
        
                        bound_pipeline.dispatch([convolved_res.max(8), convolved_res.max(8), 6]);
                    }

                    Ok(())
                });
            }

            self.ibl_resources_prepared = false;
        }

        (sh_buffer, prefilter_cubemap)
    }

    pub fn clean(self, rhi: &Rhi) {
        // let convolved_cubemap = Arc::try_unwrap(self.convolved_cubemap)
        //     .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));
        let sh_buffer = Arc::try_unwrap(self.sh_buffer)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));
        let prefilter_cubemap = Arc::try_unwrap(self.prefilter_cubemap)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));

        //rhi.device.destroy_image(convolved_cubemap);
        rhi.device.destroy_buffer(sh_buffer);
        rhi.device.destroy_image(prefilter_cubemap);
    }
}