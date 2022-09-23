use vk_sync::AccessType::{*};
use ash::vk;

pub use vk_sync::AccessType;

use super::{CommandBuffer, Device, Image, Buffer};

#[inline]
pub fn is_read_only_access(access: &AccessType) -> bool {
    match &access {
	    CommandBufferReadNVX |
        IndirectBuffer |
        IndexBuffer |
        VertexBuffer |
        VertexShaderReadUniformBuffer |
        VertexShaderReadSampledImageOrUniformTexelBuffer |
        VertexShaderReadOther |
        TessellationControlShaderReadUniformBuffer |
        TessellationControlShaderReadSampledImageOrUniformTexelBuffer |
        TessellationControlShaderReadOther |
        TessellationEvaluationShaderReadUniformBuffer |
        TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer |
        TessellationEvaluationShaderReadOther |
        GeometryShaderReadUniformBuffer |
        GeometryShaderReadSampledImageOrUniformTexelBuffer |
        GeometryShaderReadOther |
        FragmentShaderReadUniformBuffer |
        FragmentShaderReadSampledImageOrUniformTexelBuffer |
        FragmentShaderReadColorInputAttachment |
        FragmentShaderReadDepthStencilInputAttachment |
        FragmentShaderReadOther |
        ColorAttachmentRead |
        DepthStencilAttachmentRead |
        ComputeShaderReadUniformBuffer |
        ComputeShaderReadSampledImageOrUniformTexelBuffer |
        ComputeShaderReadOther |
        AnyShaderReadUniformBuffer |
        AnyShaderReadUniformBufferOrVertexBuffer |
        AnyShaderReadSampledImageOrUniformTexelBuffer |
        AnyShaderReadOther |
        TransferRead |
        HostRead |
        Present => true,
        _ => false
    }
}

#[inline]
pub fn is_write_only_access(access: &AccessType) -> bool {
    match &access {
        CommandBufferWriteNVX |
        VertexShaderWrite |
        TessellationControlShaderWrite |
        TessellationEvaluationShaderWrite |
        GeometryShaderWrite |
        FragmentShaderWrite |
        ColorAttachmentWrite |
        DepthStencilAttachmentWrite |
        DepthAttachmentWriteStencilReadOnly |
        StencilAttachmentWriteDepthReadOnly |
        ComputeShaderWrite |
        AnyShaderWrite |
        TransferWrite |
        HostWrite => true,
        _ => false
    }
}

#[inline]
pub fn is_raster_access(access: &AccessType) -> bool {
    match &access {
        ColorAttachmentRead |
        DepthStencilAttachmentRead |
        ColorAttachmentWrite |
        DepthStencilAttachmentWrite |
        DepthAttachmentWriteStencilReadOnly |
        StencilAttachmentWriteDepthReadOnly => true,
        _ => false
    }
}

#[inline]
pub fn is_read_only_raster_access(access: &AccessType) -> bool {
    match &access {
        ColorAttachmentRead |
        DepthStencilAttachmentRead => true,
        _ => false
    }
}

#[inline]
pub fn is_write_only_raster_access(access: &AccessType) -> bool {
    match &access {
        ColorAttachmentWrite |
        DepthStencilAttachmentWrite |
        DepthAttachmentWriteStencilReadOnly |
        StencilAttachmentWriteDepthReadOnly => true,
        _ => false
    }
}

// copy from vk_sync
pub struct AccessInfo {
	pub stage_mask: ash::vk::PipelineStageFlags,
	pub access_mask: ash::vk::AccessFlags,
	pub image_layout: ash::vk::ImageLayout,
}

// copy from vk_sync
pub fn get_access_info(access_type: AccessType) -> AccessInfo {
	match access_type {
		AccessType::Nothing => AccessInfo {
			stage_mask: vk::PipelineStageFlags::empty(),
			access_mask: vk::AccessFlags::empty(),
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::CommandBufferReadNVX => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
			access_mask: vk::AccessFlags::COMMAND_PREPROCESS_READ_NV,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::IndirectBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::DRAW_INDIRECT,
			access_mask: vk::AccessFlags::INDIRECT_COMMAND_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::IndexBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_INPUT,
			access_mask: vk::AccessFlags::INDEX_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_INPUT,
			access_mask: vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::VertexShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationControlShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::TessellationControlShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::TessellationControlShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationEvaluationShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer => {
			AccessInfo {
				stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
				access_mask: vk::AccessFlags::SHADER_READ,
				image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			}
		}
		AccessType::TessellationEvaluationShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::GeometryShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::GeometryShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::GeometryShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::FragmentShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadColorInputAttachment => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadDepthStencilInputAttachment => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentRead => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthStencilAttachmentRead => AccessInfo {
			stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::ComputeShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::ComputeShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::AnyShaderReadUniformBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::UNIFORM_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::AnyShaderReadUniformBufferOrVertexBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::UNIFORM_READ | vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::AnyShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TransferRead => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TRANSFER,
			access_mask: vk::AccessFlags::TRANSFER_READ,
			image_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
		},
		AccessType::HostRead => AccessInfo {
			stage_mask: vk::PipelineStageFlags::HOST,
			access_mask: vk::AccessFlags::HOST_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::Present => AccessInfo {
			stage_mask: vk::PipelineStageFlags::empty(),
			access_mask: vk::AccessFlags::empty(),
			image_layout: vk::ImageLayout::PRESENT_SRC_KHR,
		},
		AccessType::CommandBufferWriteNVX => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
			access_mask: vk::AccessFlags::COMMAND_PREPROCESS_WRITE_NV,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationControlShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationEvaluationShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::GeometryShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::FragmentShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
			image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthStencilAttachmentWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
			image_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthAttachmentWriteStencilReadOnly => AccessInfo {
			stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
				| vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::StencilAttachmentWriteDepthReadOnly => AccessInfo {
			stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
				| vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::DEPTH_READ_ONLY_STENCIL_ATTACHMENT_OPTIMAL,
		},
		AccessType::ComputeShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::AnyShaderWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::SHADER_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::TransferWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::TRANSFER,
			access_mask: vk::AccessFlags::TRANSFER_WRITE,
			image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
		},
		AccessType::HostWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::HOST,
			access_mask: vk::AccessFlags::HOST_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentReadWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
				| vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
			image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::General => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::RayTracingShaderReadColorInputAttachment => AccessInfo {
			stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
			access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::RayTracingShaderReadDepthStencilInputAttachment => AccessInfo {
			stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
			access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::RayTracingShaderReadAccelerationStructure => AccessInfo {
			stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
			access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::RayTracingShaderReadOther => AccessInfo {
			stage_mask: vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
			access_mask: vk::AccessFlags::SHADER_READ,
			image_layout: vk::ImageLayout::GENERAL,
		},
		AccessType::AccelerationStructureBuildWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
			access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::AccelerationStructureBuildRead => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
			access_mask: vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
		AccessType::AccelerationStructureBufferWrite => AccessInfo {
			stage_mask: vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
			access_mask: vk::AccessFlags::TRANSFER_WRITE,
			image_layout: vk::ImageLayout::UNDEFINED,
		},
	}
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct ImageBarrier<'a> {
	pub image: &'a Image,
	pub prev_access: &'a [AccessType],
	pub next_access: &'a [AccessType],
	pub aspect_mask: vk::ImageAspectFlags,
	#[builder(default = "false")]
	pub discard_contents: bool,
}

impl<'a> ImageBarrier<'a> {
	pub fn builder() -> ImageBarrierBuilder<'a> {
		Default::default()
	}
}

pub fn image_barrier(device: &Device, cb: &CommandBuffer, barrier: &[ImageBarrier]) {
	let img_barriers = barrier.iter()
		.map(|barrier| {
			let subresource_range = vk::ImageSubresourceRange::builder()
				.aspect_mask(barrier.aspect_mask)
				.base_array_layer(0)
				.base_mip_level(0)
				.layer_count(vk::REMAINING_ARRAY_LAYERS)
				.level_count(vk::REMAINING_MIP_LEVELS)
				.build();

			vk_sync::ImageBarrier {
				previous_accesses: barrier.prev_access,
				next_accesses: barrier.next_access,
				// always use optimal to gain max performance
				previous_layout: vk_sync::ImageLayout::Optimal,
				next_layout: vk_sync::ImageLayout::Optimal,

				discard_contents: barrier.discard_contents,
				// for now, no queue resource ownership transfer
				src_queue_family_index: device.global_queue.family.index,
				dst_queue_family_index: device.global_queue.family.index,

				image: barrier.image.raw,
				range: subresource_range,
			}
		})
		.collect::<Vec::<_>>();

	vk_sync::cmd::pipeline_barrier(
		&device.raw, 
		cb.raw,
		None,
		&[],
		&img_barriers,
	);
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct BufferBarrier<'a> {
	pub buffer: &'a Buffer,
	pub prev_access: &'a [AccessType],
	pub next_access: &'a [AccessType],
}

impl<'a> BufferBarrier<'a> {
	pub fn builder() -> BufferBarrierBuilder<'a> {
		Default::default()
	}
}

pub fn buffer_barrier(device: &Device, cb: &CommandBuffer, barrier: &[BufferBarrier]) {
	let buf_barriers = barrier.iter()
		.map(|barrier| {
			vk_sync::BufferBarrier {
				previous_accesses: barrier.prev_access,
				next_accesses: barrier.next_access,

				src_queue_family_index: device.global_queue.family.index,
				dst_queue_family_index: device.global_queue.family.index,

				buffer: barrier.buffer.raw,
				size: barrier.buffer.desc.size,
				offset: 0,
			}
		})
		.collect::<Vec::<_>>();

	vk_sync::cmd::pipeline_barrier(
		&device.raw, 
		cb.raw,
		None,
		&buf_barriers,
		&[],
	);
}

// TODO: add global barrier