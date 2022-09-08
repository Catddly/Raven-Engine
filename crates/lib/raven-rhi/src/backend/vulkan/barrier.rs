use vk_sync::AccessType::{self, *};

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
			stage_mask: ash::vk::PipelineStageFlags::empty(),
			access_mask: ash::vk::AccessFlags::empty(),
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::CommandBufferReadNVX => AccessInfo {
            // modified!
			stage_mask: ash::vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
            // modified!
			access_mask: ash::vk::AccessFlags::COMMAND_PREPROCESS_READ_NV,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::IndirectBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::DRAW_INDIRECT,
			access_mask: ash::vk::AccessFlags::INDIRECT_COMMAND_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::IndexBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_INPUT,
			access_mask: ash::vk::AccessFlags::INDEX_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_INPUT,
			access_mask: ash::vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::VertexShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationControlShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::TessellationControlShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::TessellationControlShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationEvaluationShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::TessellationEvaluationShaderReadSampledImageOrUniformTexelBuffer => {
			AccessInfo {
				stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
				access_mask: ash::vk::AccessFlags::SHADER_READ,
				image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			}
		}
		AccessType::TessellationEvaluationShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::GeometryShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::GeometryShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::GeometryShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::FragmentShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::FragmentShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadColorInputAttachment => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadDepthStencilInputAttachment => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::INPUT_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::FragmentShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentRead => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: ash::vk::AccessFlags::COLOR_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthStencilAttachmentRead => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::ComputeShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::ComputeShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::AnyShaderReadUniformBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::AnyShaderReadUniformBufferOrVertexBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::UNIFORM_READ
				| ash::vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		},
		AccessType::AnyShaderReadOther => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::SHADER_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TransferRead => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TRANSFER,
			access_mask: ash::vk::AccessFlags::TRANSFER_READ,
			image_layout: ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
		},
		AccessType::HostRead => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::HOST,
			access_mask: ash::vk::AccessFlags::HOST_READ,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::Present => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::empty(),
			access_mask: ash::vk::AccessFlags::empty(),
			image_layout: ash::vk::ImageLayout::PRESENT_SRC_KHR,
		},
		AccessType::CommandBufferWriteNVX => AccessInfo {
            // modified!
			stage_mask: ash::vk::PipelineStageFlags::COMMAND_PREPROCESS_NV,
            // modified!
			access_mask: ash::vk::AccessFlags::COMMAND_PREPROCESS_WRITE_NV,
			image_layout: ash::vk::ImageLayout::UNDEFINED,
		},
		AccessType::VertexShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::VERTEX_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationControlShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TessellationEvaluationShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::GeometryShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::GEOMETRY_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::FragmentShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::FRAGMENT_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
			image_layout: ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthStencilAttachmentWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
			image_layout: ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
		},
		AccessType::DepthAttachmentWriteStencilReadOnly => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
				| ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
		},
		AccessType::StencilAttachmentWriteDepthReadOnly => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
				| ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
			access_mask: ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
				| ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
			image_layout: ash::vk::ImageLayout::DEPTH_READ_ONLY_STENCIL_ATTACHMENT_OPTIMAL,
		},
		AccessType::ComputeShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::AnyShaderWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::SHADER_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::TransferWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::TRANSFER,
			access_mask: ash::vk::AccessFlags::TRANSFER_WRITE,
			image_layout: ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
		},
		AccessType::HostWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::HOST,
			access_mask: ash::vk::AccessFlags::HOST_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
		AccessType::ColorAttachmentReadWrite => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
			access_mask: ash::vk::AccessFlags::COLOR_ATTACHMENT_READ
				| ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
			image_layout: ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
		},
		AccessType::General => AccessInfo {
			stage_mask: ash::vk::PipelineStageFlags::ALL_COMMANDS,
			access_mask: ash::vk::AccessFlags::MEMORY_READ | ash::vk::AccessFlags::MEMORY_WRITE,
			image_layout: ash::vk::ImageLayout::GENERAL,
		},
	}
}