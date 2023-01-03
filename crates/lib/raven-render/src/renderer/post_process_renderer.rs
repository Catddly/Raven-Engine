use std::sync::Arc;

use ash::vk;

use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable, RenderGraphPassBinding};
use raven_rhi::{backend::{Image, Buffer, BufferDesc, AccessType}, Rhi};

const LUMINANCE_HISTOGRAM_BIN_COUNT: usize = 256;
pub const LUMINANCE_HISTOGRAM_MIN_LOG2: f64 = -16.0;
pub const LUMINANCE_HISTOGRAM_MAX_LOG2: f64 =  16.0;

pub const ENABLE_AUTO_EXPOSURE: bool = false;

pub fn blur_pyramid(rg: &mut RenderGraphBuilder, input_img: &RgHandle<Image>) -> RgHandle<Image> {
    let mut output_img_desc = input_img.desc()
        .half_resolution() // start with mipmap level 1
        .format(vk::Format::B10G11R11_UFLOAT_PACK32)
        .full_mipmap_levels()
        .usage_flags(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE);
    
    // skip one bottom mip level
    output_img_desc.mip_levels = output_img_desc.mip_levels
        .overflowing_sub(1)
        .0
        .max(1);

    let mut pyramid = rg.new_resource(
        output_img_desc
    );

    {
        let mut pass = rg.add_pass("blur pyramid 0");
        let pipeline = pass.register_compute_pipeline("post_processing/blur_pyramid.hlsl");
        
        let input_ref = pass.read(input_img, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
        let output_ref = pass.write(&mut pyramid, AccessType::ComputeShaderWrite);

        pass.render(move |ctx| {
            // mipmap 0
            let mut output = output_ref.bind();
            output.with_base_mipmap_level(0);

            let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                .descriptor_set(0, &[
                    input_ref.bind(),
                    output,
                ])
            )?;

            bound_pipeline.dispatch(output_img_desc.extent);

            Ok(())
        });
    }

    for mip_level in 1..output_img_desc.mip_levels {
        let mut pass = rg.add_pass(format!("blur pyramid {}", mip_level).as_str());
        let input_ref = pass.read(&pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
        let output_ref = pass.write(&mut pyramid, AccessType::ComputeShaderWrite);

        // TODO: is this will generate verbose pipelines?
        let pipeline = pass.register_compute_pipeline("post_processing/blur_pyramid.hlsl");

        pass.render(move |ctx| {
            let mut input = input_ref.bind();
            input.with_base_mipmap_level(mip_level - 1);
    
            let mut output = output_ref.bind();
            output.with_base_mipmap_level(mip_level);

            let downsample_amount = 1 << mip_level;
            let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                .descriptor_set(0, &[
                    input,
                    output,
                ])
            )?;
    
            bound_pipeline.dispatch(output_img_desc.divide_extent([downsample_amount, downsample_amount, 1]).extent);

            Ok(())
        });
    }
    
    pyramid
}

pub fn reverse_blur_pyramid(rg: &mut RenderGraphBuilder, in_pyramid: &RgHandle<Image>) -> RgHandle<Image> {
    let mut rev_pyramid = rg.new_resource(*in_pyramid.desc());

    // start from the max mipmap level
    for target_mip in (0..(rev_pyramid.desc().mip_levels as u32 - 1)).rev() {
        let downsample_amount = 1 << target_mip;
        let output_extent: [u32; 3] = rev_pyramid.desc()
            .divide_extent([downsample_amount, downsample_amount, 1])
            .extent;

        let src_mip: u32 = target_mip + 1;

        let self_weight = if src_mip == rev_pyramid.desc().mip_levels as u32 {
            0.0f32
        } else {
            0.5f32
        };

        {
            let mut pass = rg.add_pass(format!("rev blur pyramid {}", src_mip).as_str());

            let in_pyramid_ref = pass.read(&in_pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let in_rev_ref = pass.read(&rev_pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let out_rev_ref = pass.write(&mut rev_pyramid, AccessType::ComputeShaderWrite);
    
            // TODO: is this will generate verbose pipelines?
            let pipeline = pass.register_compute_pipeline("post_processing/reverse_blur_pyramid.hlsl");
            
        let push_values = (output_extent[0], output_extent[1], self_weight);
            pass.render(move |ctx| {
                let offset = ctx.global_dynamic_buffer().push(&push_values);

                let mut in_pyramid = in_pyramid_ref.bind();
                in_pyramid.with_base_mipmap_level(target_mip as u16);
                in_pyramid.with_mipmap_level_count(1);
        
                let mut in_rev = in_rev_ref.bind();
                in_rev.with_base_mipmap_level(src_mip as u16);
                in_rev.with_mipmap_level_count(1);

                let mut out_rev = out_rev_ref.bind();
                out_rev.with_base_mipmap_level(target_mip as u16);
                out_rev.with_mipmap_level_count(1);
    
                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        in_pyramid,
                        in_rev,
                        out_rev,
                        RenderGraphPassBinding::DynamicBuffer(offset)
                    ])
                )?;
        
                bound_pipeline.dispatch(output_extent);
    
                Ok(())
            });
        }
    }

    rev_pyramid
}

pub struct PostProcessRenderer {
    luminance_histogram_buffer: Arc<Buffer>,
    image_log2_luminance: f32,
}

impl PostProcessRenderer {    
    pub fn new(rhi: &Rhi) -> Self {
        let lum_buffer = rhi.device.create_buffer(BufferDesc::new_gpu_to_cpu(
                std::mem::size_of::<u32>() * LUMINANCE_HISTOGRAM_BIN_COUNT,
                vk::BufferUsageFlags::STORAGE_BUFFER
            ), "luminance histogram buffer")
            .expect("Failed to create luminance histogram buffer!");

        Self {
            luminance_histogram_buffer: Arc::new(lum_buffer),
            image_log2_luminance: 0.0,
        }
    }

    /// Readback the luminance histogram data calculated from compute shader.
    fn readback_histogram_buffer(&mut self) {
        let mut histogram_data = [0_u32; LUMINANCE_HISTOGRAM_BIN_COUNT];

        let bytes = match self.luminance_histogram_buffer.allocation.mapped_slice() {
            Some(bytes) => {
                bytemuck::checked::cast_slice::<u8, u32>(bytes)
            }
            None => {
                return;
            }
        };

        histogram_data.copy_from_slice(bytes);

        // See https://knarkowicz.wordpress.com/2016/01/09/automatic-exposure/
        // We are not simply averaging all luminance values.
        // Reject the top and bottom part of the histogram, and averaging only the middle part can help making auto-exposure more stable,
        // and helps to focus exposure on something important.

        // TODO: expose this to user
        // for now 60% of dark pixels and 10% of the bright pixels.
        let histogram_lo_clip = 0.6_f64.min(1.0);
        let histogram_hi_clip = 0.1_f64.min(1.0 - histogram_lo_clip);

        // calculate the mean part of the valid histogram counts
        let total_count = histogram_data.iter().copied().sum::<u32>();
        let mut left_reject_count = (total_count as f64 * histogram_lo_clip) as u32;
        let mut left_valid_count = (total_count as f64 * (1.0 - histogram_lo_clip - histogram_hi_clip)) as u32;

        let left_valid_count_test = left_valid_count;

        let mut valid_sum = 0.0;
        let mut valid_count_sum = 0; 

        for (bin_idx, count) in histogram_data.into_iter().enumerate() {
            // calculate the weight of this bin
            let t = (bin_idx as f64 + 0.5) / LUMINANCE_HISTOGRAM_BIN_COUNT as f64;
            // get the valid count in this bin
            let bin_valid_count = count.saturating_sub(left_reject_count).min(left_valid_count);

            left_reject_count = left_reject_count.saturating_sub(count);
            left_valid_count = left_valid_count.saturating_sub(bin_valid_count);

            valid_sum += t * bin_valid_count as f64;
            valid_count_sum += bin_valid_count;
        }

        assert_eq!(valid_count_sum, left_valid_count_test);

        let mean_lum = valid_sum / valid_count_sum.max(1) as f64;
        self.image_log2_luminance = (LUMINANCE_HISTOGRAM_MIN_LOG2 + mean_lum * (LUMINANCE_HISTOGRAM_MAX_LOG2 - LUMINANCE_HISTOGRAM_MIN_LOG2)) as f32;

        //glog::debug!("mean lum: {}", mean_lum);
        //glog::debug!("image log2 lum: {}", self.image_log2_luminance);
    }

    fn calculate_image_lum_histogram(&mut self,
        rg: &mut RenderGraphBuilder,
        blur_pyramid: &RgHandle<Image>
    ) -> RgHandle<Buffer> {
        // Use blur (gaussian blur) pyramid image to estimate the luminance of current scene.
        // This will be consistent, the bigger resolution the origin image is, the same resolution image it will choose. (in 16 : 9, around 240 * 135)
        let used_mip_level = blur_pyramid.desc().mip_levels
            .saturating_sub(7);

        let used_mip_extent = blur_pyramid.desc()
            .divide_up_extent([1 << used_mip_level, 1 << used_mip_level, 1])
            .extent;
        
        // clean the old data
        let mut temp_histogram_buf = rg.new_resource(BufferDesc::new_gpu_only(
            std::mem::size_of::<u32>() * LUMINANCE_HISTOGRAM_BIN_COUNT, vk::BufferUsageFlags::STORAGE_BUFFER)
        );

        {
            let mut pass = rg.add_pass("clear luminance histogram");

            let pipeline = pass.register_compute_pipeline("post_processing/luminance_histogram/luminance_histogram_clear.hlsl");
            let histogram_buf_ref = pass.write(&mut temp_histogram_buf, AccessType::ComputeShaderWrite);

            pass.render(move |ctx| {
                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[histogram_buf_ref.bind()])
                )?;
                
                bound_pipeline.dispatch([LUMINANCE_HISTOGRAM_BIN_COUNT as u32, 1, 1]);

                Ok(())
            });
        }

        {
            let mut pass = rg.add_pass("calculate luminance histogram");

            let pipeline = pass.register_compute_pipeline("post_processing/luminance_histogram/luminance_histogram_calculate.hlsl");
            let input_img_ref = pass.read(&blur_pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let histogram_buf_ref = pass.write(&mut temp_histogram_buf, AccessType::ComputeShaderWrite);

            let extent = [used_mip_extent[0], used_mip_extent[1]];
            pass.render(move |ctx| {
                let offset = ctx.global_dynamic_buffer().push(&extent);

                let mut input_bind = input_img_ref.bind();
                input_bind.with_base_mipmap_level(used_mip_level);

                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        input_bind,
                        histogram_buf_ref.bind(),
                        RenderGraphPassBinding::DynamicBuffer(offset)
                    ])
                )?;
                
                bound_pipeline.dispatch(used_mip_extent);

                Ok(())
            });
        }

        let mut histogram_buf = rg.import(self.luminance_histogram_buffer.clone(), AccessType::Nothing);

        {
            let mut pass = rg.add_pass("copy luminance histogram");

            let pipeline = pass.register_compute_pipeline("post_processing/luminance_histogram/luminance_histogram_copy.hlsl");
            let src_ref = pass.read(&temp_histogram_buf, AccessType::ComputeShaderReadUniformBuffer);
            let dst_ref = pass.write(&mut histogram_buf, AccessType::ComputeShaderWrite);

            pass.render(move |ctx| {
                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        src_ref.bind(),
                        dst_ref.bind()
                    ])
                )?;
                
                bound_pipeline.dispatch([LUMINANCE_HISTOGRAM_BIN_COUNT as u32, 1, 1]);

                Ok(())
            });
        }

        temp_histogram_buf
    }

    fn bloom(&self, rg: &mut RenderGraphBuilder, input_img: &RgHandle<Image>, threshold: f32) -> RgHandle<Image> {
        let skip_n_first_mip = 2;
        let downsample_amount = [1 << skip_n_first_mip, 1 << skip_n_first_mip, 1];

        let mut output_desc = input_img.desc()
            .divide_up_extent(downsample_amount)
            .format(vk::Format::B10G11R11_UFLOAT_PACK32)
            .full_mipmap_levels();
        
        let skip_n_bottom_mip = (4 - skip_n_first_mip).max(0);
        output_desc.mip_levels = output_desc.mip_levels.saturating_sub(skip_n_bottom_mip).max(1);

        let mut output = rg.new_resource(output_desc);

        let temp_output_desc = output_desc.mipmap_level(1);
        let mut temp_output = rg.new_resource(temp_output_desc);

        let extent = output_desc.extent;
        {
            let mut pass = rg.add_pass("bloom extract bright spot");
            let pipeline = pass.register_compute_pipeline("post_processing/bloom/extract_bright_spot.hlsl");

            let input_ref = pass.read(&input_img, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let output_ref = pass.write(&mut temp_output, AccessType::ComputeShaderWrite);

            let push_values = (extent[0], extent[1], threshold);

            pass.render(move |ctx| {
                let offset = ctx.global_dynamic_buffer().push(&push_values);

                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        input_ref.bind(),
                        output_ref.bind(),
                        RenderGraphPassBinding::DynamicBuffer(offset)
                    ])
                )?;

                bound_pipeline.dispatch(extent);

                Ok(())
            });
        }

        {
            let mut pass = rg.add_pass("bloom erosion");
            let pipeline = pass.register_compute_pipeline("post_processing/bloom/erosion.hlsl");

            let input_ref = pass.read(&temp_output, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let output_ref = pass.write(&mut output, AccessType::ComputeShaderWrite);

            pass.render(move |ctx| {
                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        input_ref.bind(),
                        output_ref.bind(),
                    ])
                )?;

                bound_pipeline.dispatch(extent);

                Ok(())
            });
        }

        for target_mip in 1..output_desc.mip_levels {
            let mut pass = rg.add_pass(format!("bloom blur {}", target_mip).as_str());
            let pipeline = pass.register_compute_pipeline("post_processing/bloom/bloom_blur.hlsl");

            let input_ref = pass.read(&output, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let output_ref = pass.write(&mut output, AccessType::ComputeShaderWrite);

            let downsample_amount = [1 << target_mip, 1 << target_mip, 1];
            let extent = output.desc().divide_up_extent(downsample_amount).extent;

            pass.render(move |ctx| {
                let mut input_binding = input_ref.bind();
                input_binding.with_base_mipmap_level(target_mip - 1);

                let mut output_binding = output_ref.bind();
                output_binding.with_base_mipmap_level(target_mip);

                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        input_binding,
                        output_binding,
                    ])
                )?;

                bound_pipeline.dispatch(extent);

                Ok(())
            });
        }

        output
    }

    pub fn image_log2_luminance(&self) -> f32 {
        self.image_log2_luminance
    }

    pub fn prepare_rg(
        &mut self,
        rg: &mut RenderGraphBuilder,
        input_image: RgHandle<Image>,
        post_exposure_mult: f32,
        contrast: f32,
    ) -> RgHandle<Image> {
        let mut output = rg.new_resource(input_image.desc().format(vk::Format::B10G11R11_UFLOAT_PACK32));

        let bloom_output = self.bloom(rg, &input_image, 0.92);
        let bloom_mip_level = bloom_output.desc().mip_levels as u32;

        let input_extent = input_image.desc().extent;

        if ENABLE_AUTO_EXPOSURE {
            self.readback_histogram_buffer();
    
            let blur_pyramid = blur_pyramid(rg, &input_image);
            let temp_histogram_buf = self.calculate_image_lum_histogram(rg, &blur_pyramid);
            let reverse_blur_pyramid = reverse_blur_pyramid(rg, &blur_pyramid);
    
            {
                let mut pass = rg.add_pass("post process");
                let pipeline = pass.register_compute_pipeline("post_processing/post_combine.hlsl");
    
                let input_ref = pass.read(&input_image, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let bloom_ref = pass.read(&bloom_output, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let pyramid_ref = pass.read(&blur_pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let rev_pyramid_ref = pass.read(&reverse_blur_pyramid, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let histogram_ref = pass.read(&temp_histogram_buf, AccessType::ComputeShaderReadUniformBuffer);
    
                let output_ref = pass.write(&mut output, AccessType::ComputeShaderWrite);
    
                let push_values = (
                    input_extent[0] as f32, input_extent[1] as f32, 1.0 / input_extent[0] as f32, 1.0 / input_extent[1] as f32,
                    post_exposure_mult, contrast, 1_u32, bloom_mip_level,
                );
    
                pass.render(move|ctx| {
                    let offset = ctx.global_dynamic_buffer().push(&push_values);
    
                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            input_ref.bind(),
                            output_ref.bind(),
                            RenderGraphPassBinding::DynamicBuffer(offset),
                            bloom_ref.bind(),
                            pyramid_ref.bind(),
                            rev_pyramid_ref.bind(),
                            histogram_ref.bind(),
                        ])
                    )?;
    
                    bound_pipeline.dispatch(input_extent);
    
                    Ok(())
                });
            }
    
            output
        } else {
            let mut pass = rg.add_pass("post process");
            let pipeline = pass.register_compute_pipeline("post_processing/post_combine.hlsl");

            let input_ref = pass.read(&input_image, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let bloom_ref = pass.read(&bloom_output, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let output_ref = pass.write(&mut output, AccessType::ComputeShaderWrite);

            let push_values = (
                input_extent[0] as f32, input_extent[1] as f32, 1.0 / input_extent[0] as f32, 1.0 / input_extent[1] as f32,
                1.0_f32, 1.0_f32, 0_u32, bloom_mip_level,
            );

            pass.render(move|ctx| {
                let offset = ctx.global_dynamic_buffer().push(&push_values);

                let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                    .descriptor_set(0, &[
                        input_ref.bind(),
                        output_ref.bind(),
                        RenderGraphPassBinding::DynamicBuffer(offset),
                        bloom_ref.bind(),
                    ])
                )?;

                bound_pipeline.dispatch(input_extent);

                Ok(())
            });

            output
        }
    }

    pub fn clean(self, rhi: &Rhi) {
        let histogram_buffer = Arc::try_unwrap(self.luminance_histogram_buffer)
            .unwrap_or_else(|_| panic!("Failed to release histogram buffer, someone is still using it!"));

        rhi.device.destroy_buffer(histogram_buffer);
    }
}