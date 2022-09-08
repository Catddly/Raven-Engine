use ash::vk;

pub struct RenderPass {
    raw: vk::RenderPass,
    // TODO: cache its associated frame buffers.
}