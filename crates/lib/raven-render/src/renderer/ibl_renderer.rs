use std::sync::Arc;

use ash::vk;

use raven_core::utility::as_byte_slice_values;
use raven_rg::{RenderGraphBuilder, RgHandle, RenderGraphPassBindable, IntoPipelineDescriptorBindings};
use raven_rhi::{backend::{Image, AccessType, ImageDesc}, Rhi};

pub struct IblRenderer {
    convolved_cubemap: Arc<Image>,
    need_convolve: bool,
}

impl IblRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let image = rhi.device.create_image(ImageDesc::new_cube(64, vk::Format::R8G8B8A8_UNORM)
            .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE), None)
            .expect("Failed to create convolved cubamap!");

        Self {
            convolved_cubemap: Arc::new(image),
            need_convolve: true,
        }
    }

    pub fn convolve_if_needed(&mut self, rg: &mut RenderGraphBuilder, cubemap: &RgHandle<Image>) -> RgHandle<Image> {
        let mut convolve_cubemap = if self.need_convolve {
            rg.import(self.convolved_cubemap.clone(), AccessType::ComputeShaderWrite)
        } else {
            rg.import(self.convolved_cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer)
        };

        if self.need_convolve {
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

            self.need_convolve = false;
        }

        convolve_cubemap
    }

    pub fn clean(self, rhi: &Rhi) {
        let convolved_cubemap = Arc::try_unwrap(self.convolved_cubemap)
            .unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));

        rhi.device.destroy_image(convolved_cubemap);
    }
}