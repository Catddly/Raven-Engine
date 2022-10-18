use ash::vk;
use vk_sync::AccessType;

use raven_rhi::backend::Image;

use crate::graph_resource::Handle;
use crate::graph::RenderGraph;

pub fn clear_depth_stencil(rg: &mut RenderGraph, image: &mut Handle<Image>) {
    let mut pass = rg.add_pass("clear depth");
    let cleared_img = pass.write(image, AccessType::TransferWrite);

    pass.render(move |ctx| {
        let raw_device = &ctx.context.device.raw;
        let image = ctx.context.get_image(cleared_img.handle);

        unsafe {
            raw_device.cmd_clear_depth_stencil_image(
                ctx.cb.raw,
                image.raw,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                // TODO: expose to user
                &vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
                std::slice::from_ref(&vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    level_count: 1,
                    layer_count: 1,
                    ..Default::default()
                }),
            );
        }

        Ok(())
    });
}

pub fn clear_color(rg: &mut RenderGraph, image: &mut Handle<Image>, clear_color: [f32; 4]) {
    let mut pass = rg.add_pass("clear color");
    let cleared_img = pass.write(image, AccessType::TransferWrite);

    pass.render(move |ctx| {
        let raw_device = &ctx.context.device.raw;
        let image = ctx.context.get_image(cleared_img.handle);

        unsafe {
            raw_device.cmd_clear_color_image(
                ctx.cb.raw,
                image.raw,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                // TODO: expose to user
                &vk::ClearColorValue {
                    float32: clear_color,
                },
                std::slice::from_ref(&vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    level_count: 1,
                    layer_count: 1,
                    ..Default::default()
                }),
            );
        }

        Ok(())
    });
}