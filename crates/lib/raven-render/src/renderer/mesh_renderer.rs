use std::{sync::Arc};

use ash::vk;

use glam::{Affine3A, Quat, Vec3};
use raven_core::{asset::{asset_registry::{AssetHandle, get_runtime_asset_registry}}, utility};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBinding, image_clear};
use raven_rhi::{backend::{Device, ImageDesc, Image, BufferDesc, Buffer, renderpass, RenderPass, descriptor::update_descriptor_set_buffer, RasterPipelineDesc, PipelineShaderDesc, PipelineShaderStage, AccessType, ImageViewDesc}, Rhi, copy_engine::CopyEngine};

use crate::global_bindless_descriptor::{create_engine_global_bindless_descriptor_set};

const GBUFFER_PACK_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
const GBUFFER_DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
const GBUFFER_GEOMETRIC_NORMAL_FORMAT: vk::Format = vk::Format::A2R10G10B10_UNORM_PACK32;

const MAX_GPU_MESH_COUNT: usize = 1024;

pub enum MeshRasterScheme {
    Forward,
    Deferred,
    ForwardPlus,
}

#[derive(Copy, Clone)]
pub struct MeshHandle {
    id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct GpuMesh {
    vertex_offset: u32,
    color_offset: u32,
    uv_offset: u32,
    tangent_offset: u32,
    index_offset: u32, // do we need index_offset?
    mat_id_offset: u32,

    mat_data_offset: u32,
}

#[derive(Clone)]
pub struct UploadedMesh {
    index_buffer_offset: u32,
    index_count: u32,
}

#[derive(Clone)]
pub struct MeshInstance {
    transform: Affine3A,
    handle: MeshHandle,
}

pub struct MeshRenderer {
    // TODO: temporary, should move to the global renderer
    renderpass: Arc<RenderPass>,
    // TODO: temporary, should move to the global renderer
    bindless_descriptor: vk::DescriptorSet,

    scheme: MeshRasterScheme,
    resolution: [u32; 2],

    meshes: Vec<UploadedMesh>, // mesh data used for CPU-side to submit draw call
    mesh_instances: Vec<MeshInstance>,

    current_draw_data_offset: u64,
    draw_data_buffer: Arc<Buffer>,
    mesh_buffer: Arc<Buffer>,

    device: Arc<Device>,
}

pub struct GBuffer {
    pub packed_gbuffer: RgHandle<Image>,
    pub geometric_normal: RgHandle<Image>,
    pub depth: RgHandle<Image>,
}

pub enum MeshShadingContext {
    GBuffer(GBuffer),
    #[allow(dead_code)]
    Forward,
    #[allow(dead_code)]
    ForwardPlus,
}

impl MeshRenderer {
    pub fn new(rhi: &Rhi, scheme: MeshRasterScheme, resolution: [u32; 2]) -> Self {
        let renderpass = renderpass::create_render_pass(&rhi.device, 
            renderpass::RenderPassDesc {
                color_attachments: &[
                    // packed gbuffer
                    renderpass::RenderPassAttachmentDesc::new(GBUFFER_PACK_FORMAT).useless_input(),
                    // geometric normal
                    renderpass::RenderPassAttachmentDesc::new(GBUFFER_GEOMETRIC_NORMAL_FORMAT).useless_input(),
                ],
                depth_attachment: Some(renderpass::RenderPassAttachmentDesc::new(GBUFFER_DEPTH_FORMAT)),
            }
        );

        // create giant buffers to contain mesh vertex data and index data
        let universal_draw_data_buffer_desc: BufferDesc = BufferDesc::new_gpu_only(
            1024 * 1024 * 512, // 512 MB
            vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::INDEX_BUFFER
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            | vk::BufferUsageFlags::TRANSFER_DST
        );
        
        let universal_mesh_buffer_desc: BufferDesc = BufferDesc::new_cpu_to_gpu(
            MAX_GPU_MESH_COUNT * std::mem::size_of::<GpuMesh>(),
            vk::BufferUsageFlags::STORAGE_BUFFER
        );
        
        // Why not create this buffer inside render graph?
        // because it is a global resource we want to manage manually and it is not depend on render graph's context
        let draw_data_buffer = Arc::new(rhi.device
            .create_buffer(universal_draw_data_buffer_desc, "universal draw data buffer")
            .unwrap());
            
        let mesh_buffer = Arc::new(rhi.device
            .create_buffer(universal_mesh_buffer_desc, "universal mesh buffer")
            .unwrap());

        let bindless_descriptor = create_engine_global_bindless_descriptor_set(rhi);

        update_descriptor_set_buffer(&rhi.device, 
            0, 
            bindless_descriptor, 
            vk::DescriptorType::STORAGE_BUFFER, 
            &draw_data_buffer);

        update_descriptor_set_buffer(&rhi.device, 
            1, 
            bindless_descriptor, 
            vk::DescriptorType::STORAGE_BUFFER, 
            &mesh_buffer);

        // temporary hack
        let mut mesh_instances = Vec::new();
        mesh_instances.push(MeshInstance {
            transform: Affine3A::from_scale_rotation_translation(
                Vec3::splat(1.0),
                Quat::IDENTITY,
                Vec3::splat(0.0)
            ),
            handle: MeshHandle { id: 0 },
        });

        Self {
            renderpass,
            bindless_descriptor,

            scheme,
            resolution,

            meshes: Default::default(),
            mesh_instances,

            current_draw_data_offset: 0,
            draw_data_buffer,
            mesh_buffer,

            device: rhi.device.clone(),
        }
    }

    pub fn add_asset_mesh(&mut self, handle: &Arc<AssetHandle>) -> MeshHandle {
        // TODO: load the texture and material asset from handle.
        let registry = get_runtime_asset_registry();
        
        let mut id = u32::MAX;
        {
            let read_guard = registry.read(); 

            if let Some(asset) = read_guard.get_asset(&handle) {
                if let Some(mesh_asset) = asset.as_mesh() {
                    let curr_global_offset = self.current_draw_data_offset as u32;

                    let packed = mesh_asset.packed.as_slice();
                    let colors = mesh_asset.colors.as_slice();
                    let uvs = mesh_asset.uvs.as_slice();
                    let tangents = mesh_asset.tangents.as_slice();
                    let indices = mesh_asset.indices.as_slice();
                    let mat_ids = mesh_asset.material_ids.as_slice();

                    // copy upload
                    let mut copy_engine = CopyEngine::new();
                    
                    let packed_offset  = copy_engine.copy(&packed)   + curr_global_offset;
                    let color_offset   = copy_engine.copy(&colors)   + curr_global_offset;
                    let uv_offset      = copy_engine.copy(&uvs)      + curr_global_offset;
                    let tangent_offset = copy_engine.copy(&tangents) + curr_global_offset;
                    let index_offset   = copy_engine.copy(&indices)  + curr_global_offset;
                    let mat_id_offset  = copy_engine.copy(&mat_ids)  + curr_global_offset;

                    // Same in the asset::mod.rs
                    #[repr(C)]
                    #[derive(Copy, Clone)]
                    struct UploadMaterial {
                        metallic          : f32,
                        roughness         : f32,
                        base_color        : [f32; 4],
                        emissive          : [f32; 3],
                        texture_mapping   : [u32; 4],
                        texture_transform : [[f32; 6]; 4],
                    }

                    let mut upload_materials = Vec::new();
                    for mat_ref in mesh_asset.materials.iter() {
                        let material = read_guard.get_asset(mat_ref.handle()).unwrap().as_material().unwrap();

                        let upload = UploadMaterial {
                            metallic: material.metallic,
                            roughness: material.roughness,
                            base_color: material.base_color,
                            emissive: material.emissive,
                            texture_mapping: material.texture_mapping,
                            texture_transform: material.texture_transform,
                        };

                        upload_materials.push(upload);
                    }
                    let mat_data_offset = copy_engine.copy(&upload_materials) + curr_global_offset;

                    let totol_size_bytes = copy_engine.current_offset();
                    copy_engine.upload(
                        &self.device,
                        &self.draw_data_buffer, 
                        curr_global_offset
                    ).expect("Failed to upload mesh data with copy engine!");

                    self.current_draw_data_offset += totol_size_bytes as u64;

                    let mesh_id = self.meshes.len();

                    // upload mesh data
                    let gpu_meshes = unsafe {
                        let ptr = self.mesh_buffer.allocation.mapped_ptr().unwrap().as_ptr() as *mut GpuMesh;
                        std::slice::from_raw_parts_mut(ptr, MAX_GPU_MESH_COUNT)
                    };
                    gpu_meshes[mesh_id] = GpuMesh {
                        vertex_offset: packed_offset,
                        color_offset,
                        uv_offset,
                        tangent_offset,
                        index_offset,
                        mat_id_offset,
                        mat_data_offset,
                    };

                    self.meshes.push(UploadedMesh {
                        index_count: indices.len() as u32,
                        index_buffer_offset: index_offset,
                    });

                    id = mesh_id as u32;
                } else {
                    panic!("Trying to add a non-mesh asset in add_asset_mesh()!");
                }
            }
        };

        assert_ne!(id, u32::MAX);
        MeshHandle {
            id,
        }
    }

    pub fn prepare_rg(&mut self, rg: &mut RenderGraphBuilder) -> MeshShadingContext {
        // create shading context (GBuffer etc.)
        let mut shading_context = match self.scheme {
            MeshRasterScheme::Deferred => {
                let packed = rg.new_resource(ImageDesc::new_2d(self.resolution, GBUFFER_PACK_FORMAT));
                let geo_normal = rg.new_resource(ImageDesc::new_2d(self.resolution, GBUFFER_GEOMETRIC_NORMAL_FORMAT));
                let mut depth = rg.new_resource(ImageDesc::new_2d(self.resolution, GBUFFER_DEPTH_FORMAT));
                
                image_clear::clear_depth_stencil(rg, &mut depth);

                MeshShadingContext::GBuffer(GBuffer { 
                    packed_gbuffer: packed, 
                    geometric_normal: geo_normal, 
                    depth, 
                })
            },
            _ => unimplemented!("MeshRasterScheme"),
        };

        let extent = [self.resolution[0], self.resolution[1]];
        let renderpass = self.renderpass.clone();
        let bindless_descriptor = self.bindless_descriptor;
        {
            let mut pass = rg.add_pass("mesh raster");
            let pipeline = pass.register_raster_pipeline(&[
                PipelineShaderDesc::builder()
                    .source("defer/defer_raster.hlsl")
                    .entry("vs_main")
                    .stage(PipelineShaderStage::Vertex)
                    .build().unwrap(),
                PipelineShaderDesc::builder()
                    .source("defer/defer_raster.hlsl")
                    .entry("ps_main")
                    .stage(PipelineShaderStage::Pixel)
                    .build().unwrap()
            ], RasterPipelineDesc::builder()
                .render_pass(renderpass.clone())
                .build().unwrap()
            );

            match &mut shading_context {
                MeshShadingContext::GBuffer(gbuffer) => {
                    let depth_ref = pass.raster_write(&mut gbuffer.depth, AccessType::DepthAttachmentWriteStencilReadOnly);
                    let gbuffer_ref = pass.raster_write(&mut gbuffer.packed_gbuffer, AccessType::ColorAttachmentWrite);
                    let geo_normal_ref = pass.raster_write(&mut gbuffer.geometric_normal, AccessType::ColorAttachmentWrite);

                    let draw_data_buffer = self.draw_data_buffer.clone();
                    // TODO: this would be copied every frame, any better idea?
                    let meshes = self.meshes.to_owned();
                    let mesh_instances = self.mesh_instances.to_owned();

                    pass.render(move |ctx| {
                        let xform_iter = mesh_instances.iter()
                            .map(|ins| {
                                // transpose to column-major matrix to be used in shader
                                let transform = [
                                    ins.transform.x_axis.x,
                                    ins.transform.y_axis.x,
                                    ins.transform.z_axis.x,
                                    ins.transform.translation.x,
                                    ins.transform.x_axis.y,
                                    ins.transform.y_axis.y,
                                    ins.transform.z_axis.y,
                                    ins.transform.translation.y,
                                    ins.transform.x_axis.z,
                                    ins.transform.y_axis.z,
                                    ins.transform.z_axis.z,
                                    ins.transform.translation.z,
                                ];

                                transform
                            });
                        let instance_data_offset = ctx.global_dynamic_buffer().push_from_iter(xform_iter);

                        ctx.begin_render_pass(
                            &*renderpass, 
                            extent, 
                            &[
                                (gbuffer_ref, &ImageViewDesc::default()),
                                (geo_normal_ref, &ImageViewDesc::default())
                            ],
                            Some((depth_ref, &ImageViewDesc::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .build().unwrap()
                            ))
                        )?;
                        ctx.set_default_viewport_and_scissor(extent);

                        let bound_pipeline = ctx.bind_raster_pipeline(pipeline.into_bindings()
                            .descriptor_set(0, &[RenderGraphPassBinding::DynamicStorageBuffer(instance_data_offset)])
                            .raw_descriptor_set(1, bindless_descriptor)
                        )?;

                        // do drawing
                        for (instance_idx, mesh_ins) in mesh_instances.iter().enumerate() {
                            let mesh = &meshes[mesh_ins.handle.id as usize];

                            unsafe {
                                let raw = &ctx.device().raw;

                                raw.cmd_bind_index_buffer(
                                    ctx.cb.raw, 
                                    draw_data_buffer.raw,
                                    mesh.index_buffer_offset as u64,
                                    vk::IndexType::UINT32,
                                );
    
                                let push_constants = [mesh_ins.handle.id, instance_idx as u32];
                                bound_pipeline.push_constants(
                                    vk::ShaderStageFlags::ALL_GRAPHICS, 
                                    0,
                                    utility::as_byte_slice_values(&push_constants)
                                );

                                raw.cmd_draw_indexed(ctx.cb.raw,
                                    mesh.index_count,
                                    1, 0, 0, 0
                                );
                            }
                        }

                        ctx.end_render_pass();
                        Ok(())
                    });
                },
                _ => unimplemented!(),
            }
        }

        shading_context
    }

    pub fn clean(self, rhi: &Rhi) {
        let draw_data_buffer = Arc::try_unwrap(self.draw_data_buffer).unwrap();
        let mesh_buffer = Arc::try_unwrap(self.mesh_buffer).unwrap();

        rhi.device.destroy_buffer(draw_data_buffer);
        rhi.device.destroy_buffer(mesh_buffer);
    }
}