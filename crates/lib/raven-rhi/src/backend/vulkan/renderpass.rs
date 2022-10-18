use std::{sync::Arc, collections::{HashMap}};

use parking_lot::Mutex;
use ash::vk;
use arrayvec::ArrayVec;

use super::{Device, constants, ImageDesc};

/// Renderpass decides how to draw at a given range of time. 
/// FrameBuffer decide what to draw on at a given range of time.
/// RenderPassAttachmentDesc is all the necessary data for vulkan to know how to draw.
/// FrameBufferCacheKey has dependency on Renderpass, so it only needs to contain the data excludes from the render pass to distinguish differents FrameBuffers.

// TODO: renderpass layout transition.
// In theory, use subpass or renderpass to auto transition image layout is more efficient than using pipeline layout transition.
#[derive(Debug, Clone, Copy)]
pub struct RenderPassAttachmentDesc {
    pub format: vk::Format,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub stencil_load_op: vk::AttachmentLoadOp,
    pub stencil_store_op: vk::AttachmentStoreOp,
    pub sample: vk::SampleCountFlags,
}

impl RenderPassAttachmentDesc {
    pub fn new(format: vk::Format) -> Self {
        Self {
            format,
            load_op: vk::AttachmentLoadOp::LOAD,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            sample: vk::SampleCountFlags::TYPE_1,
        }
    }

    pub fn useless_input(mut self) -> Self {
        self.load_op = vk::AttachmentLoadOp::DONT_CARE;
        self
    }

    pub fn clear_input(mut self) -> Self {
        self.load_op = vk::AttachmentLoadOp::CLEAR;
        self
    }

    pub fn discard_output(mut self) -> Self {
        self.store_op = vk::AttachmentStoreOp::DONT_CARE;
        self
    }
}

impl RenderPassAttachmentDesc {
    pub fn to_vk(&self, initial_layout: vk::ImageLayout, final_layout: vk::ImageLayout) -> vk::AttachmentDescription {
        vk::AttachmentDescription {
            format: self.format,
            samples: self.sample,
            load_op: self.load_op,
            store_op: self.store_op,
            stencil_load_op: self.stencil_load_op,
            stencil_store_op: self.stencil_store_op,
            initial_layout,
            final_layout,
            ..Default::default()
        }
    }
}

pub struct RenderPassDesc<'a> {
    pub color_attachments: &'a [RenderPassAttachmentDesc],
    // a render pass may NOT have an depth attachment
    pub depth_attachment: Option<RenderPassAttachmentDesc>,
}

pub struct RenderPass {
    pub raw: vk::RenderPass,
    pub frame_buffer_cache: FrameBufferCache,
}

type FrameBufferAttachmentInfo = (vk::ImageCreateFlags, vk::ImageUsageFlags);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FrameBufferCacheKey {

    pub extent: [u32; 2],
    // is it necessary to identify which image belongs to its attachments?
    pub attachments: ArrayVec<FrameBufferAttachmentInfo, { constants::MAX_RENDERPASS_ATTACHMENTS + 1 }>
}

impl FrameBufferCacheKey {
    pub fn new<'a>(
        extent: [u32; 2],
        color_attachments: impl Iterator<Item = &'a ImageDesc>,
        depth_attachment: Option<&'a ImageDesc>,
    ) -> Self {
        let attachments = color_attachments
            .map(|desc| (desc.flags, desc.usage))
            .chain(depth_attachment.iter().map(|desc| (desc.flags, desc.usage)))
            .collect();

        Self {
            extent,
            attachments,
        }
    }
}

// Remember all the renderpass's information
pub struct FrameBufferCache {
    pub frame_buffer_cache: Mutex<HashMap<FrameBufferCacheKey, vk::Framebuffer>>,
    pub attachments_desc: ArrayVec<RenderPassAttachmentDesc, { constants::MAX_RENDERPASS_ATTACHMENTS + 1 }>,
    pub render_pass: vk::RenderPass,
    pub color_attachment_count: usize,
}

impl FrameBufferCache {
    pub fn new(raw: vk::RenderPass,
        color_attachments: &[RenderPassAttachmentDesc],
        depth_attachment: Option<RenderPassAttachmentDesc>,
    ) -> Self {
        let mut attachments_desc = ArrayVec::new();
        attachments_desc.try_extend_from_slice(color_attachments).unwrap();

        if let Some(depth) = depth_attachment {
            attachments_desc.push(depth);
        }

        Self {
            render_pass: raw,
            attachments_desc,
            color_attachment_count: color_attachments.len(),
            frame_buffer_cache: Default::default(),
        }
    }

    pub fn get_or_create(
        &self,
        device: &Device,
        key: FrameBufferCacheKey,
    ) -> vk::Framebuffer {
        let mut frame_buffer_cache = self.frame_buffer_cache.lock();

        if let Some(fb) = frame_buffer_cache.get(&key) {
            return *fb;
        } else {
            let (width, height) = (key.extent[0], key.extent[1]);

            let attachment_image_infos = self.attachments_desc.iter()
                .zip(key.attachments.iter())
                .map(|(desc, attach)| {
                    vk::FramebufferAttachmentImageInfo::builder()
                        .width(width)
                        .height(height)
                        .flags(attach.0)
                        .usage(attach.1)
                        .layer_count(1)
                        .view_formats(std::slice::from_ref(&desc.format))
                        .build()
                })
                .collect::<ArrayVec<_, {constants::MAX_RENDERPASS_ATTACHMENTS + 1 }>>();

            let mut fb_attach_ci = vk::FramebufferAttachmentsCreateInfo::builder()
                .attachment_image_infos(&attachment_image_infos)
                .build();

            let mut framebuffer_ci = vk::FramebufferCreateInfo::builder()
                .push_next(&mut fb_attach_ci)
                .width(width)
                .height(height)
                .render_pass(self.render_pass)
                .layers(1)
                .flags(vk::FramebufferCreateFlags::IMAGELESS) // can have no image views
                .build();
            framebuffer_ci.attachment_count = attachment_image_infos.len() as u32;

            let framebuffer = unsafe { device.raw
                .create_framebuffer(&framebuffer_ci, None)
                .expect("Failed to create vulkan framebuffer!") 
            };

            frame_buffer_cache.insert(key, framebuffer);

            return framebuffer;
        }
    }
}

pub fn create_render_pass(device: &Device, desc: RenderPassDesc<'_>) -> Arc<RenderPass> {
    assert!(!desc.color_attachments.is_empty() || (desc.color_attachments.is_empty() && desc.depth_attachment.is_some()));

    let attachments = desc.color_attachments.iter()
        .map(|color| color.to_vk(
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        )
        .chain(desc.depth_attachment.as_ref().iter()
            .map(|depth| 
                depth.to_vk(
                    vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
                    vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL)
                )
        )
        .collect::<Vec<_>>();

    let color_attachment_refs = (0..desc.color_attachments.len() as u32)
        .map(|index| vk::AttachmentReference::builder()
            .attachment(index)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()
        )
        .collect::<Vec<_>>();

    let depth_attachment_ref = vk::AttachmentReference::builder()
        .attachment(desc.color_attachments.len() as u32)
        .layout(vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL)
        .build();

    // TODO: optimize the subpass.
    // for now, each renderpass just contain one implicit subpass.
    let subpass_builder = vk::SubpassDescription::builder()
        .color_attachments(&color_attachment_refs)
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

    let subpass_desc = if desc.depth_attachment.is_some() {
        [subpass_builder.depth_stencil_attachment(&depth_attachment_ref).build()]
    } else {
        [subpass_builder.build()]
    };

    // for now, NO subpass dependencies
    let renderpass_ci = vk::RenderPassCreateInfo::builder()
        .attachments(&attachments)
        .subpasses(&subpass_desc)
        .build();

    let renderpass = unsafe { device.raw
        .create_render_pass(&renderpass_ci, None)
        .expect("Failed to create vulkan render pass!")
    };

    Arc::new(RenderPass {
        raw: renderpass,
        frame_buffer_cache: FrameBufferCache::new(renderpass, desc.color_attachments, desc.depth_attachment),
    })
}