use std::{sync::Arc, collections::BTreeMap};

use ash::vk;
use rspirv_reflect::PushConstantInfo;

use super::{RenderPass, ShaderSource, Device, ShaderBinaryStage, RHIError, descriptor::{self, PipelineSetLayouts}, PipelineShaderStage};

pub struct CommonPipeline {
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub set_layout_infos: Vec<BTreeMap<u32, vk::DescriptorType>>,
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
    pub push_constants_bytes: usize,  // push constants for the whole raster pipeline.
}

impl RasterPipelineDesc {
    pub fn builder() -> RasterPipelineDescBuilder {
        RasterPipelineDescBuilder::default()
    }
}

pub struct RasterPipeline {
    pipeline: CommonPipeline,
}

// Compute Pipeline description
#[derive(Builder, Clone, Hash, PartialEq, Eq, Debug)]
#[builder(pattern = "owned", derive(Clone))]
pub struct ComputePipelineDesc {
    #[builder(default)]
    pub push_constants_bytes: usize,
    #[builder(setter(into))]
    pub source: ShaderSource,
}

impl ComputePipelineDesc {
    pub fn builder() -> ComputePipelineDescBuilder {
        ComputePipelineDescBuilder::default()
    }
}

pub struct ComputePipeline {
    pipeline: CommonPipeline,
    dispatch_groups: [u32; 3],
}

pub fn create_raster_pipeline(
    device: &Device, 
    desc: RasterPipelineDesc, 
    shader_binaries: &[ShaderBinaryStage]
) -> anyhow::Result<RasterPipeline, RHIError> {
    let (set_layouts, push_constants): (Vec<PipelineSetLayouts>, Vec<(Option<PushConstantInfo>, PipelineShaderStage)>) = shader_binaries.iter()
        .map(|binary| {
            let reflection_data = rspirv_reflect::Reflection::new_from_spirv(&binary.binary.spirv)
                .expect("Failed to get spirv reflection data!");

            (reflection_data.get_descriptor_sets().unwrap(), (reflection_data.get_push_constant_range().unwrap(), binary.stage))
        })
        .unzip();

    let pipeline_set_layouts = descriptor::flatten_all_stages_descriptor_set_layouts(set_layouts);

    // TODO: thing of the global descriptors layout of the engine
    let (set_layout, set_layout_infos) = descriptor::create_descriptor_set_layouts_with_unified_stage(
        &device,
        &pipeline_set_layouts,
        vk::ShaderStageFlags::ALL
    ).expect("Failed to create vulkan descriptor set layout!");

    let push_constant_ranges = push_constants.into_iter()
        .filter_map(|(pc, stage)| pc.and_then(|pc| {
            Some(vk::PushConstantRange::builder()
                .size(pc.size)
                .offset(pc.offset)
                .stage_flags(match stage {
                    PipelineShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
                    PipelineShaderStage::Pixel => vk::ShaderStageFlags::FRAGMENT,
                })
                .build())
        }))
        .collect::<Vec<_>>();

    let pipeline_layout_ci = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(&set_layout)
        .push_constant_ranges(&push_constant_ranges)
        .build();

    Ok(RasterPipeline {
        pipeline: CommonPipeline {
            pipeline_layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            set_layout_infos,
            descriptor_pool_sizes: Vec::new(),
            descriptor_set_layouts: set_layout,
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        }
    })
}