use std::{sync::Arc, collections::BTreeMap, ffi::CString, ops::Deref};

use ash::vk;
use raven_container::TempList;
use rspirv_reflect::PushConstantInfo;
use byte_slice_cast::AsSliceOf;

use super::{RenderPass, ShaderSource, Device, ShaderBinaryStage, RhiError, descriptor::{self, PipelineSetLayouts}, PipelineShaderStage, ShaderBinary, constants};
use super::descriptor::PipelineSetBindings;
#[cfg(feature = "gpu_ray_tracing")]
use super::Buffer;

pub type PipelineSetLayoutInfo = BTreeMap<u32, vk::DescriptorType>;

#[derive(Copy, Clone, Debug)]
pub struct CommonPipelinePtrs {
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
}

#[derive(Debug)]
pub struct CommonPipelineInfo {
    pub set_layout_infos: Vec<PipelineSetLayoutInfo>,
    pub descriptor_pool_sizes: Vec<vk::DescriptorPoolSize>,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub pipeline_bind_point: vk::PipelineBindPoint,
}

#[derive(Debug)]
pub struct CommonPipeline {
    pub pipeline_ptrs: CommonPipelinePtrs,
    pub pipeline_info: CommonPipelineInfo,
}

impl CommonPipeline {
    #[inline]
    pub fn pipeline(&self) -> vk::Pipeline {
        self.pipeline_ptrs.pipeline
    }

    #[inline]
    pub fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_ptrs.pipeline_layout
    }

    #[inline]
    pub fn pipeline_bind_point(&self) -> vk::PipelineBindPoint {
        self.pipeline_info.pipeline_bind_point
    }
}

#[derive(Clone, Debug)]
pub enum RasterPipelinePrimitiveTopology {
    LineList,
    TriangleList,
}

#[derive(Clone, Debug)]
pub enum RasterPipelineCullMode {
    Back,
    Front,
    None,
    FrontAndBack,
}

// Raster Pipeline description
#[derive(Builder, Clone)]
#[builder(pattern = "owned", derive(Clone))]
pub struct RasterPipelineDesc {
    pub render_pass: Arc<RenderPass>,
    #[builder(default = "RasterPipelineCullMode::Back")]
    pub cull_mode: RasterPipelineCullMode,
    #[builder(default = "RasterPipelinePrimitiveTopology::TriangleList")]
    pub topology: RasterPipelinePrimitiveTopology,
    #[builder(default = "false")]
    pub depth_bias: bool,
    #[builder(default)]
    pub custom_set_layout_overwrites: [Option<PipelineSetBindings>; constants::MAX_DESCRIPTOR_SET_COUNT],
    #[builder(default = "true")]
    pub depth_test: bool,
    #[builder(default = "true")]
    pub depth_write: bool,
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
    #[builder(setter(into))]
    pub source: ShaderSource,
    #[builder(default)]
    pub custom_set_layout_overwrites: [Option<PipelineSetBindings>; constants::MAX_DESCRIPTOR_SET_COUNT],
}

impl std::hash::Hash for ComputePipelineDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl PartialEq for ComputePipelineDesc {
    fn eq(&self, other: &Self) -> bool {
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

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Copy, Clone, Debug)]
pub struct RayTracingShaderBindingTableDesc {
    pub raygen_entry_count: u32,
    pub miss_entry_count: u32,
    pub hit_entry_count: u32,
}

/// Here we define buffer for each type of ray tracing shader,
/// but not one buffer contains all the shaders.
#[cfg(feature = "gpu_ray_tracing")]
#[derive(Debug)]
pub struct RayTracingShaderBindingTable {
    pub raygen_shader_binding_table: vk::StridedDeviceAddressRegionKHR,
    pub raygen_shader_binding_table_buffer: Option<Buffer>,
    pub miss_shader_binding_table: vk::StridedDeviceAddressRegionKHR,
    pub miss_shader_binding_table_buffer: Option<Buffer>,
    pub hit_shader_binding_table: vk::StridedDeviceAddressRegionKHR,
    pub hit_shader_binding_table_buffer: Option<Buffer>,
    pub callable_shader_binding_table: vk::StridedDeviceAddressRegionKHR,
    pub callable_shader_binding_table_buffer: Option<Buffer>,
}

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Builder, Clone, Debug)]
#[builder(pattern = "owned", derive(Clone))]
pub struct RayTracingPipelineDesc {
    #[builder(default)]
    pub custom_set_layout_overwrites: [Option<PipelineSetBindings>; constants::MAX_DESCRIPTOR_SET_COUNT],
    #[builder(default = "1")]
    pub max_ray_recursive_depth: u32,
}

#[cfg(feature = "gpu_ray_tracing")]
impl RayTracingPipelineDesc {
    pub fn builder() -> RayTracingPipelineDescBuilder {
        RayTracingPipelineDescBuilder::default()
    }
}

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Debug)]
pub struct RayTracingPipeline {
    pub pipeline: CommonPipeline,
    pub sbt: RayTracingShaderBindingTable,
}

#[cfg(feature = "gpu_ray_tracing")]
impl Deref for RayTracingPipeline {
    type Target = CommonPipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

pub fn create_raster_pipeline(
    device: &Device,
    desc: RasterPipelineDesc, 
    shader_binaries: &[ShaderBinaryStage]
) -> anyhow::Result<RasterPipeline, RhiError> {
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

    let pipeline_layout_ci = if push_constant.0.is_some() {
        let push_constant_ranges = vk::PushConstantRange::builder()
            .size(push_constant.0.as_ref().unwrap().size)
            .offset(push_constant.0.as_ref().unwrap().offset)
            .stage_flags(vk::ShaderStageFlags::ALL_GRAPHICS)
            .build();

        vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layout)
            .push_constant_ranges(std::slice::from_ref(&push_constant_ranges))
            .build()
    } else {
        vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layout)
            .build()
    };

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
                    #[cfg(feature = "gpu_ray_tracing")]
                    _ => unreachable!("Creating raster pipeline, but found incorrect pipeline shader stage!")
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
        .topology(match desc.topology {
            RasterPipelinePrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            RasterPipelinePrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
        })
        .build();

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .cull_mode(match desc.cull_mode {
            RasterPipelineCullMode::Back => vk::CullModeFlags::BACK,
            RasterPipelineCullMode::Front => vk::CullModeFlags::FRONT,
            RasterPipelineCullMode::None => vk::CullModeFlags::NONE,
            RasterPipelineCullMode::FrontAndBack => vk::CullModeFlags::FRONT_AND_BACK,
        })
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .depth_bias_enable(desc.depth_bias)
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
        .depth_test_enable(desc.depth_test)
        .depth_write_enable(desc.depth_write)
        .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL) // Use reverse depth to gain better z-depth precision
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

    let dynamic_state = if desc.depth_bias {
        vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR, vk::DynamicState::DEPTH_BIAS])
            .build()
    } else {
        vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR])
            .build()
    };

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
            pipeline_ptrs: CommonPipelinePtrs { 
                pipeline_layout,
                pipeline 
            },
            pipeline_info: CommonPipelineInfo {
                set_layout_infos,
                descriptor_pool_sizes,
                descriptor_set_layouts: set_layout,
                pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            }
        }
    })
}

pub fn create_compute_pipeline(
    device: &Device,
    desc: ComputePipelineDesc,
    shader_binary: &ShaderBinary,
) -> anyhow::Result<ComputePipeline, RhiError> {
    let (mut set_layouts, push_constants, group_size) = {
        let reflection_data = rspirv_reflect::Reflection::new_from_spirv(&shader_binary.spirv)
            .expect("Failed to get spirv reflection data!");

        (reflection_data.get_descriptor_sets().expect("get sets error"), reflection_data.get_push_constant_range().unwrap(), reflection_data.get_compute_group_size().unwrap())
    };

    // force overwriting the exists set layout
    for overwrite in desc.custom_set_layout_overwrites.iter() {
        if let Some(overwrite) = overwrite {
            // is it exist?
            if let Some(layout) = set_layouts.get_mut(&1) {
                *layout = overwrite.clone();
            }
        }
    }

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
            pipeline_ptrs: CommonPipelinePtrs { 
                pipeline_layout,
                pipeline 
            },
            pipeline_info: CommonPipelineInfo {
                set_layout_infos,
                descriptor_pool_sizes,
                descriptor_set_layouts: set_layout,
                pipeline_bind_point: vk::PipelineBindPoint::COMPUTE,
            }
        },
        dispatch_groups: [group_size.0, group_size.1, group_size.2],
    })
}

#[cfg(feature = "gpu_ray_tracing")]
pub fn create_ray_tracing_pipeline(
    device: &Device,
    desc: RayTracingPipelineDesc,
    shader_binaries: &[ShaderBinaryStage]
) -> anyhow::Result<RayTracingPipeline, RhiError>  {
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

    for overwrite in desc.custom_set_layout_overwrites.iter() {
        if let Some(overwrite) = overwrite {
            // is it exist?
            if let Some(layout) = pipeline_set_layouts.get_mut(&1) {
                *layout = overwrite.clone();
            }
        }
    }

    let (set_layout, set_layout_infos) = descriptor::create_descriptor_set_layouts_with_unified_stage(
        &device,
        &pipeline_set_layouts,
        vk::ShaderStageFlags::ALL
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

    let pipeline_layout_ci = if push_constant.0.is_some() {
        let push_constant_ranges = vk::PushConstantRange::builder()
            .size(push_constant.0.as_ref().unwrap().size)
            .offset(push_constant.0.as_ref().unwrap().offset)
            .stage_flags(vk::ShaderStageFlags::ALL)
            .build();

        vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layout)
            .push_constant_ranges(std::slice::from_ref(&push_constant_ranges))
            .build()
    } else {
        vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layout)
            .build()
    };

    let pipeline_layout = unsafe { device.raw
        .create_pipeline_layout(&pipeline_layout_ci, None)
        .expect("Failed to create vulkan pipeline layout!")
    };

    let mut raygen_entry_count = 0;
    let mut miss_entry_count = 0;
    let mut hit_entry_count = 0;

    let create_shader_module_func = |shader: &ShaderBinaryStage| -> (vk::ShaderModule, String) {
        let shader_module_ci = vk::ShaderModuleCreateInfo::builder()
            .code(shader.binary.as_ref().unwrap().spirv.as_slice_of::<u32>().unwrap())
            .build();

        let shader_module = unsafe { device.raw
            .create_shader_module(&shader_module_ci, None)
            .expect("Failed to create ray tracing shader module!")
        };

        (shader_module, shader.entry.clone())
    };

    // each vk::RayTracingShaderGroupCreateInfoKHR will eventually used as sbt entry
    let mut shader_groups_ci: Vec<vk::RayTracingShaderGroupCreateInfoKHR> = Vec::new();
    // same as other pipeline, but this will be compiled to shader handles
    // and we can query the handles by calling vkGetRayTracingShaderGroupHandlesKHR()
    let mut shader_stages_ci: Vec<vk::PipelineShaderStageCreateInfo> = Vec::new();

    let mut prev_stage: Option<PipelineShaderStage> = None;

    let mut temp_entry_points = Vec::new();

    // Preserve shader sequence
    // RayGen(general) -> RayMiss(general) -> RayCHit || RayAHit

    for shader in shader_binaries {
        let group_id = shader_stages_ci.len();

        match shader.stage {
            PipelineShaderStage::RayGen => {
                assert!(prev_stage == None || prev_stage == Some(PipelineShaderStage::RayGen));
                raygen_entry_count += 1;

                let (module, entry_point) = create_shader_module_func(shader);

                temp_entry_points.push(CString::new(entry_point).unwrap());
                let entry_point = &**temp_entry_points.last().unwrap();

                let stage = vk::PipelineShaderStageCreateInfo::builder()
                    .stage(vk::ShaderStageFlags::RAYGEN_KHR)
                    .module(module)
                    .name(entry_point)
                    .build();

                let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                    .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                    .general_shader(group_id as _)
                    .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                    .any_hit_shader(vk::SHADER_UNUSED_KHR)
                    .intersection_shader(vk::SHADER_UNUSED_KHR)
                    .build();

                shader_stages_ci.push(stage);
                shader_groups_ci.push(group);
            }
            PipelineShaderStage::RayMiss => {
                assert!(prev_stage == Some(PipelineShaderStage::RayGen) || prev_stage == Some(PipelineShaderStage::RayMiss));
                miss_entry_count += 1;

                let (module, entry_point) = create_shader_module_func(shader);

                temp_entry_points.push(CString::new(entry_point).unwrap());
                let entry_point = &**temp_entry_points.last().unwrap();

                let stage = vk::PipelineShaderStageCreateInfo::builder()
                    .stage(vk::ShaderStageFlags::MISS_KHR)
                    .module(module)
                    .name(entry_point)
                    .build();

                let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                    .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                    .general_shader(group_id as _)
                    .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                    .any_hit_shader(vk::SHADER_UNUSED_KHR)
                    .intersection_shader(vk::SHADER_UNUSED_KHR)
                    .build();

                shader_stages_ci.push(stage);
                shader_groups_ci.push(group);
            }
            PipelineShaderStage::RayClosestHit => {
                assert!(prev_stage == Some(PipelineShaderStage::RayMiss) || prev_stage == Some(PipelineShaderStage::RayClosestHit));
                hit_entry_count += 1;

                let (module, entry_point) = create_shader_module_func(shader);

                temp_entry_points.push(CString::new(entry_point).unwrap());
                let entry_point = &**temp_entry_points.last().unwrap();

                let stage = vk::PipelineShaderStageCreateInfo::builder()
                    .stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                    .module(module)
                    .name(entry_point)
                    .build();

                let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                    .ty(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP)
                    .general_shader(vk::SHADER_UNUSED_KHR)
                    .closest_hit_shader(group_id as _)
                    .any_hit_shader(vk::SHADER_UNUSED_KHR)
                    .intersection_shader(vk::SHADER_UNUSED_KHR)
                    .build();

                shader_stages_ci.push(stage);
                shader_groups_ci.push(group);
            }
            // TODO: support AnyHit shader
            _ => unreachable!("Creating ray tracing pipeline, but found incorrect pipeline shader stage!")
        }

        prev_stage = Some(shader.stage);
    }

    assert!(raygen_entry_count > 0);
    assert!(miss_entry_count > 0);

    let pipeline_raw = unsafe { device.ray_tracing_extensions.ray_tracing_pipeline_khr
        .create_ray_tracing_pipelines(
            vk::DeferredOperationKHR::null(),
            vk::PipelineCache::null(),
            &[vk::RayTracingPipelineCreateInfoKHR::builder()
                    .stages(&shader_stages_ci)
                    .groups(&shader_groups_ci)
                    .max_pipeline_ray_recursion_depth(desc.max_ray_recursive_depth) // TODO
                    .layout(pipeline_layout)
                    .build()
            ],
            None)
        .expect("Failed to create vulkan ray tracing pipeline!")[0]
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

    let sbt = device.create_ray_tracing_shader_binding_table(
        RayTracingShaderBindingTableDesc {
            raygen_entry_count,
            hit_entry_count,
            miss_entry_count,
        },
        pipeline_raw,
    )?;

    Ok(RayTracingPipeline {
        pipeline: CommonPipeline {
            pipeline_ptrs: CommonPipelinePtrs { 
                pipeline_layout,
                pipeline: pipeline_raw,
            },
            pipeline_info: CommonPipelineInfo {
                set_layout_infos,
                descriptor_pool_sizes,
                descriptor_set_layouts: set_layout,
                pipeline_bind_point: vk::PipelineBindPoint::RAY_TRACING_KHR,
            }
        },
        sbt,
    })
}

pub fn destroy_raster_pipeline(device: &Device, pipeline: RasterPipeline) {
    destroy_common_pipeline_ptrs(device, pipeline.pipeline.pipeline_ptrs);
}

pub fn destroy_compute_pipeline(device: &Device, pipeline: ComputePipeline) {
    destroy_common_pipeline_ptrs(device, pipeline.pipeline.pipeline_ptrs);
}

#[cfg(feature = "gpu_ray_tracing")]
pub fn destroy_ray_tracing_pipeline(device: &Device, pipeline: RayTracingPipeline) {
    device.destroy_ray_tracing_shader_binding_table(pipeline.sbt);
    destroy_common_pipeline_ptrs(device, pipeline.pipeline.pipeline_ptrs);
}

#[inline]
pub(crate) fn destroy_common_pipeline_ptrs(device: &Device, pipeline_ptrs: CommonPipelinePtrs) {
    unsafe {
        device.raw
            .destroy_pipeline_layout(pipeline_ptrs.pipeline_layout, None);

        device.raw
            .destroy_pipeline(pipeline_ptrs.pipeline, None);
    }
}