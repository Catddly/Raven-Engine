use std::{sync::Arc, collections::BTreeMap, ffi::CString, ops::Deref};

use ash::vk;
use raven_core::container::TempList;
use rspirv_reflect::PushConstantInfo;
use byte_slice_cast::AsSliceOf;

use super::{RenderPass, ShaderSource, Device, ShaderBinaryStage, RHIError, descriptor::{self, PipelineSetLayouts}, PipelineShaderStage, ShaderBinary, constants};
use super::descriptor::PipelineSetBindings;

pub type PipelineSetLayoutInfo = BTreeMap<u32, vk::DescriptorType>;

#[derive(Debug)]
pub struct CommonPipeline {
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub set_layout_infos: Vec<PipelineSetLayoutInfo>,
    pub descriptor_pool_sizes: Vec<vk::DescriptorPoolSize>,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub pipeline_bind_point: vk::PipelineBindPoint,
}

// Raster Pipeline description
#[derive(Builder, Clone)]
#[builder(pattern = "owned", derive(Clone))]
pub struct RasterPipelineDesc {
    pub render_pass: Arc<RenderPass>,
    #[builder(default = "true")]
    pub culling: bool,
    #[builder(default = "true")]
    pub depth_write: bool,
    #[builder(default)]
    pub custom_set_layout_overwrites: [Option<PipelineSetBindings>; constants::MAX_DESCRIPTOR_SET_COUNT],
}

impl RasterPipelineDesc {
    pub fn builder() -> RasterPipelineDescBuilder {
        RasterPipelineDescBuilder::default()
    }
}

#[derive(Debug)]
pub struct RasterPipeline {
    pub pipeline: CommonPipeline,
}

impl Deref for RasterPipeline {
    type Target = CommonPipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

// Compute Pipeline description
#[derive(Builder, Clone, Debug)]
#[builder(pattern = "owned", derive(Clone))]
pub struct ComputePipelineDesc {
    #[builder(default)]
    pub push_constants_bytes: usize,
    #[builder(setter(into))]
    pub source: ShaderSource,
    #[builder(default)]
    pub custom_set_layout_overwrites: [Option<PipelineSetBindings>; constants::MAX_DESCRIPTOR_SET_COUNT],
}

impl std::hash::Hash for ComputePipelineDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.push_constants_bytes);
        self.source.hash(state);
    }
}

impl PartialEq for ComputePipelineDesc {
    fn eq(&self, other: &Self) -> bool {
        self.push_constants_bytes.eq(&other.push_constants_bytes) &&
            self.source.eq(&other.source)
    }
}
impl Eq for ComputePipelineDesc {}

impl ComputePipelineDesc {
    pub fn builder() -> ComputePipelineDescBuilder {
        ComputePipelineDescBuilder::default()
    }
}

#[derive(Debug)]
pub struct ComputePipeline {
    pub pipeline: CommonPipeline,
    pub dispatch_groups: [u32; 3],
}

impl Deref for ComputePipeline {
    type Target = CommonPipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

pub fn create_raster_pipeline(
    device: &Device, 
    desc: RasterPipelineDesc, 
    shader_binaries: &[ShaderBinaryStage]
) -> anyhow::Result<RasterPipeline, RHIError> {
    let (set_layouts, push_constants): (Vec<PipelineSetLayouts>, Vec<(Option<PushConstantInfo>, PipelineShaderStage)>) = shader_binaries.iter()
        .map(|binary| {
            let reflection_data = rspirv_reflect::Reflection::new_from_spirv(&binary.binary.as_ref().unwrap().spirv)
                .expect("Failed to get spirv reflection data!");

            (reflection_data.get_descriptor_sets().unwrap(), (reflection_data.get_push_constant_range().unwrap(), binary.stage))
        })
        .unzip();
    
    let mut pipeline_set_layouts = descriptor::flatten_all_stages_descriptor_set_layouts(set_layouts);

    // force overwriting the exists set layout
    for overwrite in desc.custom_set_layout_overwrites.iter() {
        if let Some(overwrite) = overwrite {
            // is it exist?
            if let Some(layout) = pipeline_set_layouts.get_mut(&1) {
                *layout = overwrite.clone();
            }
        }
    }

    //glog::debug!("Raster pipeline layout: {:#?}", pipeline_set_layouts);

    // TODO: thing of the global descriptors layout of the engine
    let (set_layout, set_layout_infos) = descriptor::create_descriptor_set_layouts_with_unified_stage(
        &device,
        &pipeline_set_layouts,
        vk::ShaderStageFlags::ALL_GRAPHICS
    ).expect("Failed to create vulkan descriptor set layout!");

    // merge push constants into a single one (the layout must be the same!)
    let push_constant = push_constants.iter()
        .reduce(|lhs, rhs| {
            match (lhs.0.is_some(), rhs.0.is_some()) {
                (true, true) => {
                    assert_eq!(lhs.0.as_ref().unwrap().size, rhs.0.as_ref().unwrap().size);                
                    assert_eq!(lhs.0.as_ref().unwrap().offset, rhs.0.as_ref().unwrap().offset);

                    lhs
                },
                (true, false) => {
                    lhs
                },
                (false, true) | (false, false) => {
                    rhs
                },
            }
        })
        .unwrap();

    let push_constant_ranges = vk::PushConstantRange::builder()
        .size(push_constant.0.as_ref().unwrap().size)
        .offset(push_constant.0.as_ref().unwrap().offset)
        .stage_flags(vk::ShaderStageFlags::ALL_GRAPHICS)
        .build();

    let pipeline_layout_ci = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(&set_layout)
        .push_constant_ranges(std::slice::from_ref(&push_constant_ranges))
        .build();

    let pipeline_layout = unsafe { device.raw
        .create_pipeline_layout(&pipeline_layout_ci, None)
        .expect("Failed to create vulkan pipeline layout!")
    };

    // contain the CStr which will be dropped inside the scope, and lead to badly formatted CStr error inside vulkan.
    let temp_name = TempList::new();
    let shader_modules = shader_binaries.iter()
        .map(|binary| {
            let shader_module_ci = vk::ShaderModuleCreateInfo::builder()
                .code(binary.binary.as_ref().unwrap().spirv.as_slice_of::<u32>().unwrap())
                .build();

            let shader_module = unsafe { device.raw
                .create_shader_module(&shader_module_ci, None)
                .expect("Failed to create vulkan shader module")
            };

            // create pipeline shader modules
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(match binary.stage {
                    PipelineShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
                    PipelineShaderStage::Pixel => vk::ShaderStageFlags::FRAGMENT,
                })
                .module(shader_module)
                .name(temp_name.add(CString::new(binary.entry.as_str()).unwrap()))
                .build()
        })
        .collect::<Vec<_>>();

    // We do NOT need any vertex input bindings, because we use buffer address to find vertex and index buffer data
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&[])
        .vertex_attribute_descriptions(&[])
        .build();

    let vertex_input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .build();

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .cull_mode(if desc.culling { 
            vk::CullModeFlags::BACK
        } else {
            vk::CullModeFlags::NONE
        })
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .build();

    // don't specified viewport and scissor here, bind dynamically
    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .scissor_count(1)
        .viewport_count(1)
        .build();

    let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .build();

    let noop_stencil_op = vk::StencilOpState::builder()
        .fail_op(vk::StencilOp::KEEP)
        .pass_op(vk::StencilOp::KEEP)
        .depth_fail_op(vk::StencilOp::KEEP)
        .compare_op(vk::CompareOp::ALWAYS)
        .build();

    let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_test_enable(true)
        .depth_write_enable(desc.depth_write)
        .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL) // Use depth reverse to gain better z-depth precision
        .front(noop_stencil_op)
        .back(noop_stencil_op)
        .max_depth_bounds(1.0)
        .build();

    let attachments = vec![
        vk::PipelineColorBlendAttachmentState::builder()
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::SRC_COLOR)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::all())
            .build();
        desc.render_pass.frame_buffer_cache.color_attachment_count
    ];

    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
        .attachments(&attachments)
        .build();

    let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
        .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR])
        .build();

    let graphic_pipeline_ci = vk::GraphicsPipelineCreateInfo::builder()
        .stages(&shader_modules)
        .layout(pipeline_layout)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&vertex_input_assembly_state)
        .rasterization_state(&rasterization_state)
        .viewport_state(&viewport_state)
        .multisample_state(&multisample_state)
        .depth_stencil_state(&depth_stencil_state)
        .color_blend_state(&color_blend_state)
        .dynamic_state(&dynamic_state)
        .render_pass(desc.render_pass.raw)
        .build();

    let pipeline = unsafe { device.raw
        // TODO: add pipeline cache
        .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipeline_ci], None)
        .expect("Failed to create vulkan graphic pipeline!")[0]
    };
    
    // store its descriptors infos into the pipeline
    let mut descriptor_pool_sizes: Vec<vk::DescriptorPoolSize> = Vec::new();
    for bindings in &set_layout_infos {
        for ty in bindings.values() {
            if let Some(pool_size) = descriptor_pool_sizes.iter_mut().find(|pool_size| pool_size.ty == *ty) {
                pool_size.descriptor_count += 1;
            } else {
                descriptor_pool_sizes.push(vk::DescriptorPoolSize::builder()
                    .ty(*ty)
                    .descriptor_count(1)
                    .build());
            }
        }
    }

    Ok(RasterPipeline {
        pipeline: CommonPipeline {
            pipeline_layout,
            pipeline,
            set_layout_infos,
            descriptor_pool_sizes,
            descriptor_set_layouts: set_layout,
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        }
    })
}

pub fn destroy_raster_pipeline(device: &Device, pipeline: RasterPipeline) {
    destroy_pipeline_common(device, pipeline.pipeline);
}

pub fn destroy_compute_pipeline(device: &Device, pipeline: ComputePipeline) {
    destroy_pipeline_common(device, pipeline.pipeline);
}

#[inline]
fn destroy_pipeline_common(device: &Device, pipeline: CommonPipeline) {
    unsafe {
        device.raw
            .destroy_pipeline(pipeline.pipeline, None);
    }
}

pub fn create_compute_pipeline(
    device: &Device,
    shader_binary: &ShaderBinary,
) -> anyhow::Result<ComputePipeline, RHIError> {
    let (set_layouts, push_constants, group_size) = {
        let reflection_data = rspirv_reflect::Reflection::new_from_spirv(&shader_binary.spirv)
            .expect("Failed to get spirv reflection data!");

        (reflection_data.get_descriptor_sets().expect("get sets error"), reflection_data.get_push_constant_range().unwrap(), reflection_data.get_compute_group_size().unwrap())
    };

    // TODO: thing of the global descriptors layout of the engine
    let (set_layout, set_layout_infos) = descriptor::create_descriptor_set_layouts_with_unified_stage(
        &device,
        &set_layouts,
        vk::ShaderStageFlags::COMPUTE
    ).expect("Failed to create vulkan descriptor set layout!");

    let pipeline_layout_builder = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(&set_layout);

    let pipeline_layout = if push_constants.is_some() {
        // merge push constants into a single one (the layout must be the same!)
        let push_constant = push_constants.iter()
            .reduce(|lhs, rhs| {
                assert_eq!(lhs.size, rhs.size);                
                assert_eq!(lhs.offset, rhs.offset);

                lhs
            })
            .unwrap();

        let push_constant_ranges = vk::PushConstantRange::builder()
            .size(push_constant.size)
            .offset(push_constant.offset)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .build();

        let pipeline_layout_ci = pipeline_layout_builder
            .push_constant_ranges(std::slice::from_ref(&push_constant_ranges))
            .build();

        unsafe { device.raw
            .create_pipeline_layout(&pipeline_layout_ci, None)
            .expect("Failed to create vulkan pipeline layout!")
        }
    } else {
        let pipeline_layout_ci = pipeline_layout_builder.build();

        unsafe { device.raw
            .create_pipeline_layout(&pipeline_layout_ci, None)
            .expect("Failed to create vulkan pipeline layout!")
        }
    };

    let temp_names = TempList::new();
    let shader_module = {
        let shader_module_ci = vk::ShaderModuleCreateInfo::builder()
            .code(shader_binary.spirv.as_slice_of::<u32>().unwrap())
            .build();

        let shader_module = unsafe { device.raw
            .create_shader_module(&shader_module_ci, None)
            .expect("Failed to create vulkan shader module")
        };

        vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(temp_names.add(CString::new("main").unwrap()))
            .build()
    };

    let compute_pipeline_ci = vk::ComputePipelineCreateInfo::builder()
        .layout(pipeline_layout)
        .stage(shader_module)
        .build();

    let pipeline = unsafe { device.raw
        // TODO: add pipeline cache
        .create_compute_pipelines(vk::PipelineCache::null(), &[compute_pipeline_ci], None)
        .expect("Failed to create vulkan graphic pipeline!")[0]
    };

    let mut descriptor_pool_sizes: Vec<vk::DescriptorPoolSize> = Vec::new();
    for bindings in set_layout_infos.iter() {
        for ty in bindings.values() {
            if let Some(mut dps) = descriptor_pool_sizes.iter_mut().find(|item| item.ty == *ty)
            {
                dps.descriptor_count += 1;
            } else {
                descriptor_pool_sizes.push(vk::DescriptorPoolSize {
                    ty: *ty,
                    descriptor_count: 1,
                })
            }
        }
    }

    Ok(ComputePipeline {
        pipeline: CommonPipeline {
            pipeline_layout,
            pipeline,
            set_layout_infos,
            descriptor_pool_sizes,
            descriptor_set_layouts: set_layout,
            pipeline_bind_point: vk::PipelineBindPoint::COMPUTE,
        },
        dispatch_groups: [group_size.0, group_size.1, group_size.2],
    })
}