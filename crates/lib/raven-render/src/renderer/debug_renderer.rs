use std::sync::Arc;

use ash::vk;

use glam::{Vec3, Mat4};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBinding, RenderGraphPassBindable};
use raven_core::{math::AABB, utility};
use raven_rhi::{backend::{
    Device,
    RasterPipelineDesc, PipelineShaderDesc, PipelineShaderStage,
    Image, Buffer, RenderPass, renderpass, RenderPassDesc, RenderPassAttachmentDesc, AccessType, ImageViewDesc, BufferDesc, RasterPipelinePrimitiveTopology, RasterPipelineCullMode
}, Rhi};

const MAX_DEBUG_AABBS: usize = 32;

pub struct DebugRenderer {
    debug_aabbs: Vec<AABB>,

    draw_data_buffer: Arc<Buffer>,
    line_lists_buffers: Vec<Arc<Buffer>>,

    renderpass: Arc<RenderPass>,
    device: Arc<Device>,
}

impl DebugRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let renderpass = renderpass::create_render_pass(
            &rhi.device,
            RenderPassDesc {
                color_attachments: &[
                    // input from post processing
                    RenderPassAttachmentDesc::new(vk::Format::B10G11R11_UFLOAT_PACK32)
                ],
                depth_attachment: Some(RenderPassAttachmentDesc::new(vk::Format::D32_SFLOAT))
            }
        );

        let boxes_vb_data = generate_debug_box_vb_data();

        let vb_buffer = rhi.device.create_buffer_init(
            BufferDesc::new_gpu_only(
                boxes_vb_data.len() * std::mem::size_of::<Vec3>(),
                vk::BufferUsageFlags::STORAGE_BUFFER |
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            ),
            "debug renderer vb",
            &boxes_vb_data
        )
        .expect("Failed to create vertex buffer for debug renderer!");

        Self {
            debug_aabbs: Vec::new(),

            draw_data_buffer: Arc::new(vb_buffer),
            line_lists_buffers: Vec::new(),

            renderpass,
            device: rhi.device.clone(),
        }
    }

    pub fn add_debug_aabb(&mut self, aabb: AABB) {
        assert!(self.debug_aabbs.len() <= MAX_DEBUG_AABBS);

        self.debug_aabbs.push(aabb);
    }

    pub fn add_debug_line_lists(&mut self, line_lists: Vec<Vec3>) {
        assert!(line_lists.len() % 2 == 0 && !line_lists.is_empty());

        let line_lists_buffer = self.device.create_buffer_init(
            BufferDesc::new_gpu_only(
                std::mem::size_of::<Vec3>() * line_lists.len(),
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER
            ),
            "debug line lists",
            &line_lists
        )
        .expect("Failed to create debug line lists buffer!");

        self.line_lists_buffers.push(Arc::new(line_lists_buffer));
    }

    pub fn remove_all_aabbs(&mut self) {
        self.debug_aabbs.clear();
    }

    pub fn prepare_rg(&mut self,
        rg: &mut RenderGraphBuilder,
        input: &mut RgHandle<Image>,
        depth: &mut RgHandle<Image>
    ) {
        let draw_data_buffer = rg.import(self.draw_data_buffer.clone(), AccessType::Nothing);
        let line_lists_buffer = self.line_lists_buffers.iter()
            .map(|line_list| {
                rg.import(line_list.clone(), AccessType::Nothing)
            })
            .collect::<Vec<_>>();

        let mut pass = rg.add_pass("debug overlay");
        let pipeline = pass.register_raster_pipeline(
        &[
            PipelineShaderDesc::builder()
                .source("debug/debug_draw_lines.hlsl")
                .stage(PipelineShaderStage::Vertex)
                .entry("vs_main")
                .build().unwrap(),
            PipelineShaderDesc::builder()
                .source("debug/debug_draw_lines.hlsl")
                .stage(PipelineShaderStage::Pixel)
                .entry("ps_main")
                .build().unwrap(),
        ],
        RasterPipelineDesc::builder()
            .render_pass(self.renderpass.clone())
            .cull_mode(RasterPipelineCullMode::None)
            .depth_write(false)
            .topology(RasterPipelinePrimitiveTopology::LineList)
            .build().unwrap()
        );

        let line_lists_ref = line_lists_buffer.iter()
            .map(|handle| {
                pass.read(handle, AccessType::AnyShaderReadOther)
            })
            .collect::<Vec<_>>();

        let draw_data_ref = pass.read(&draw_data_buffer, AccessType::AnyShaderReadOther);
        let depth_ref = pass.raster_write(depth, AccessType::DepthAttachmentWriteStencilReadOnly);
        let input_ref = pass.raster_write(input, AccessType::ColorAttachmentWrite);

        let renderpass = self.renderpass.clone();
        let extent = input.desc().extent;
        let extent = [extent[0], extent[1]];

        let debug_aabb_matrices = self.debug_aabbs.iter()
            .map(|aabb| {
                let matrix = Mat4::from_translation(aabb.get_center()) * Mat4::from_scale(aabb.get_extent());

                [
                    matrix.x_axis.x,
                    matrix.y_axis.x,
                    matrix.z_axis.x,
                    matrix.w_axis.x,

                    matrix.x_axis.y,
                    matrix.y_axis.y,
                    matrix.z_axis.y,
                    matrix.w_axis.y,

                    matrix.x_axis.z,
                    matrix.y_axis.z,
                    matrix.z_axis.z,
                    matrix.w_axis.z,

                    matrix.x_axis.w,
                    matrix.y_axis.w,
                    matrix.z_axis.w,
                    matrix.w_axis.w,
                ]
            })
            .collect::<Vec<_>>();

        pass.render(move |ctx| {
            let debug_aabb_count = debug_aabb_matrices.len() as u32;
            let debug_aabb_matrices_offset = ctx.global_dynamic_buffer().push_from_iter(debug_aabb_matrices.into_iter());
            
            let identity_matrix: [f32; 16] = [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ];
            let line_lists_offset = ctx.global_dynamic_buffer().push(&identity_matrix);

            ctx.begin_render_pass(
                &renderpass,
                extent,
                &[
                    (input_ref, &ImageViewDesc::default()),
                ],
                Some((
                    depth_ref,
                    &ImageViewDesc::builder().aspect_mask(vk::ImageAspectFlags::DEPTH).build().unwrap()
                ))
            )?;
            ctx.set_default_viewport_and_scissor(extent);

            let bound_pipeline = ctx.bind_raster_pipeline(pipeline.into_bindings()
                .descriptor_set(0, &[
                    RenderGraphPassBinding::DynamicStorageBuffer(debug_aabb_matrices_offset),
                    draw_data_ref.bind()
                ])
            )?;

            for i in 0..debug_aabb_count {
                bound_pipeline.push_constants(
                    vk::ShaderStageFlags::ALL_GRAPHICS,
                    0,
                    utility::as_byte_slice(&i)
                );

                let raw_device = &ctx.device().raw;

                unsafe {
                    raw_device.cmd_draw(
                        ctx.cb.raw,
                        12 * 2,
                        1,
                        0,
                        0
                    );
                }
            }

            for line_list_ref in line_lists_ref {
                bound_pipeline.rebind(0, &[
                    RenderGraphPassBinding::DynamicStorageBuffer(line_lists_offset),
                    line_list_ref.bind()
                ])?;
    
                bound_pipeline.push_constants(
                    vk::ShaderStageFlags::ALL_GRAPHICS,
                    0,
                    utility::as_byte_slice(&0)
                );

                let raw_device = &ctx.device().raw;

                unsafe {
                    raw_device.cmd_draw(
                        ctx.cb.raw,
                        12 * 2,
                        1,
                        0,
                        0
                    );
                }
            }

            ctx.end_render_pass();

            Ok(())
        });
    }

    pub fn clean(self) {
        let draw_data_buffer = Arc::try_unwrap(self.draw_data_buffer)
            .expect("Reference count of debug renderer draw data buffer may not be retained!");
            
        self.device.destroy_buffer(draw_data_buffer);

        for line_list_buffer in self.line_lists_buffers {
            let line_list_buffer = Arc::try_unwrap(line_list_buffer)
                .expect("Reference count of debug renderer line list buffer may not be retained!");
            
            self.device.destroy_buffer(line_list_buffer);
        }
    }
}

fn generate_debug_box_vb_data() -> Vec<Vec3> {
    let mut out_vertices = Vec::new();
    // line [0]
    out_vertices.push(Vec3::from((-1.0, -1.0, -1.0)));
    out_vertices.push(Vec3::from(( 1.0, -1.0, -1.0)));
    // line [1]
    out_vertices.push(Vec3::from((-1.0, -1.0, -1.0)));
    out_vertices.push(Vec3::from((-1.0,  1.0, -1.0)));
    // line [2]
    out_vertices.push(Vec3::from((-1.0, -1.0, -1.0)));
    out_vertices.push(Vec3::from((-1.0, -1.0,  1.0)));
    // line [3]
    out_vertices.push(Vec3::from(( 1.0,  1.0, -1.0)));
    out_vertices.push(Vec3::from(( 1.0,  1.0,  1.0)));
    // line [4]
    out_vertices.push(Vec3::from(( 1.0,  1.0, -1.0)));
    out_vertices.push(Vec3::from(( 1.0, -1.0, -1.0)));
    // line [5]
    out_vertices.push(Vec3::from(( 1.0,  1.0, -1.0)));
    out_vertices.push(Vec3::from((-1.0,  1.0, -1.0)));
    // line [6]
    out_vertices.push(Vec3::from((-1.0,  1.0,  1.0)));
    out_vertices.push(Vec3::from(( 1.0,  1.0,  1.0)));
    // line [7]
    out_vertices.push(Vec3::from((-1.0,  1.0,  1.0)));
    out_vertices.push(Vec3::from((-1.0, -1.0,  1.0)));
    // Line [8]
    out_vertices.push(Vec3::from((-1.0,  1.0,  1.0)));
    out_vertices.push(Vec3::from((-1.0,  1.0, -1.0)));
    // Line [9]
    out_vertices.push(Vec3::from(( 1.0, -1.0,  1.0)));
    out_vertices.push(Vec3::from(( 1.0,  1.0,  1.0)));
    // Line [10]
    out_vertices.push(Vec3::from(( 1.0, -1.0,  1.0)));
    out_vertices.push(Vec3::from((-1.0, -1.0,  1.0)));
    // Line [11]
    out_vertices.push(Vec3::from(( 1.0, -1.0,  1.0)));
    out_vertices.push(Vec3::from(( 1.0, -1.0, -1.0)));

    out_vertices
}