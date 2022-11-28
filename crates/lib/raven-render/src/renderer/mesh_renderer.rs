use std::{sync::Arc};

use ash::vk;

use glam::{Affine3A};
use raven_core::{asset::{asset_registry::{AssetHandle, get_runtime_asset_registry}, PackedVertex, VecArrayQueryParam}, utility};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBinding, image_clear, create_engine_global_bindless_descriptor_set};
use raven_rhi::{backend::{Device, ImageDesc, Image, BufferDesc, Buffer, renderpass, RenderPass, descriptor::update_descriptor_set_buffer, RasterPipelineDesc, PipelineShaderDesc, PipelineShaderStage, AccessType, ImageViewDesc, ImageSubresource}, Rhi, copy_engine::CopyEngine};

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

pub struct BindlessTexHandle(u32);

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

#[derive(Copy, Clone)]
pub struct MeshInstanceHandle(u32);

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
    bindless_tex_sizes_buffer: Buffer,

    bindless_images: Vec<Arc<Image>>,
    next_bindless_texture_idx: u32,

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

// Same in the asset::mod.rs
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct UploadMaterial {
    metallic          : f32,
    roughness         : f32,
    base_color        : [f32; 4],
    emissive          : [f32; 3],
    texture_mapping   : [u32; 4],
    texture_transform : [[f32; 6]; 4],
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

        let texture_sizes_buffer_desc: BufferDesc = BufferDesc::new_cpu_to_gpu(
            rhi.device.max_bindless_descriptor_count() as usize * std::mem::size_of::<[f32; 4]>(),
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

        let bindless_tex_sizes_buffer = rhi.device
            .create_buffer(texture_sizes_buffer_desc, "bindless textures sizes")
            .unwrap();

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

        update_descriptor_set_buffer(&rhi.device, 
            2, 
            bindless_descriptor, 
            vk::DescriptorType::STORAGE_BUFFER, 
            &bindless_tex_sizes_buffer);

        let mesh_instances = Vec::new();

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
            bindless_tex_sizes_buffer,

            bindless_images: Vec::new(),
            next_bindless_texture_idx: 0,

            device: rhi.device.clone(),
        }
    }

    // TODO: this function should not be here!
    pub fn add_bindless_texture(&mut self, handle: &Arc<AssetHandle>) -> BindlessTexHandle {
        let registry = get_runtime_asset_registry();
        {
            let read_guard = registry.read();
            
            if let Some(asset) = read_guard.get_asset(&handle) {
                let (extent, img_subresources) = if let Some(tex_asset) = asset.as_texture() {
                    // TODO: identify image type
                    let uploads = tex_asset.lod_groups.iter()
                        .map(|mip| ImageSubresource {
                            data: mip.as_slice(),
                            row_pitch_in_bytes: tex_asset.extent[0] * 4,
                            base_layer: 0,
                        })
                        .collect::<Vec<_>>();

                    (tex_asset.extent, uploads)
                } else if let Some(baked_tex) = asset.as_baked() {
                    let tex_field_reader = read_guard.get_baked_texture_asset(baked_tex);
                    let extent = tex_field_reader.extent();

                    let lod_length = tex_field_reader.lod_groups(VecArrayQueryParam::length()).length();
                    let mut uploads = Vec::with_capacity(lod_length);

                    for i in 0..lod_length {
                        let lod = tex_field_reader.lod_groups(VecArrayQueryParam::index(i)).array();

                        uploads.push(ImageSubresource {
                            data: lod,
                            // TODO: no hardcode
                            row_pitch_in_bytes: extent[0] * 4,
                            base_layer: 0,
                        });
                    }

                    (extent, uploads)
                } else {
                    panic!("Expect texture asset handle!");
                };

                let extent_inv_extent = [
                    extent[0] as f32, extent[1] as f32,
                    (extent[0] as f32).recip(), (extent[1] as f32).recip()
                ];

                // create gpu image
                let image_desc = ImageDesc::new_2d([extent[0], extent[1]], vk::Format::R8G8B8A8_UNORM)
                    .mipmap_level(img_subresources.len() as _)
                    .usage_flags(vk::ImageUsageFlags::SAMPLED);
                let image = Arc::new(self.device.create_image(image_desc, Some(img_subresources)).unwrap());
                let image_view = image.view(&self.device, &ImageViewDesc::default()).unwrap();

                let handle = self.add_bindless_image_view(image_view);
                self.bindless_images.push(image);

                // update sizes infos
                bytemuck::checked::cast_slice_mut::<u8, [f32; 4]>(
                    self.bindless_tex_sizes_buffer
                        .allocation
                        .mapped_slice_mut()
                        .unwrap()
                )[handle.0 as usize] = extent_inv_extent;

                return handle;
            }
        }

        BindlessTexHandle(u32::MAX)
    }

    fn add_bindless_image_view(&mut self, view: vk::ImageView) -> BindlessTexHandle {
        let handle = BindlessTexHandle(self.next_bindless_texture_idx);
        self.next_bindless_texture_idx += 1;

        // upload this bindless image
        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(view)
            .build();

        let write = vk::WriteDescriptorSet::builder()
            .dst_set(self.bindless_descriptor)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .dst_binding(3)
            .dst_array_element(handle.0)
            .image_info(std::slice::from_ref(&image_info))
            .build();

        unsafe {
            self.device
                .raw
                .update_descriptor_sets(std::slice::from_ref(&write), &[]);
        }

        handle
    }

    pub fn add_asset_mesh(&mut self, handle: &Arc<AssetHandle>) -> MeshHandle {
        // TODO: load the texture asset from handle.
        let registry = get_runtime_asset_registry();
        {
            let read_guard = registry.read(); 

            if let Some(asset) = read_guard.get_asset(&handle) {
                if let Some(mesh_asset) = asset.as_mesh() {
                    let packed = mesh_asset.packed.as_slice();
                    let colors = mesh_asset.colors.as_slice();
                    let uvs = mesh_asset.uvs.as_slice();
                    let tangents = mesh_asset.tangents.as_slice();
                    let indices = mesh_asset.indices.as_slice();
                    let mat_ids = mesh_asset.material_ids.as_slice();

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

                    return self.upload_gpu_mesh_data(packed, colors, uvs, tangents, indices, mat_ids, &upload_materials);
                } else if let Some(baked) = asset.as_baked() {
                    let field_reader = read_guard.get_baked_mesh_asset(baked);

                    let packed = field_reader.packed();
                    let colors = field_reader.colors();
                    let uvs = field_reader.uvs();
                    let tangents = field_reader.tangents();
                    let indices = field_reader.indices();
                    let mat_ids = field_reader.material_ids();

                    let mat_refs = read_guard.get_asset_relative_materials(handle)
                        .expect(format!("Failed to get mesh relative materials: {:?}", handle).as_str());
                    let mut upload_materials = Vec::with_capacity(mat_refs.len());

                    for mat_ref in mat_refs.iter() {
                        let mat_asset = read_guard.get_asset(mat_ref.handle()).unwrap();
                        let baked_mat = mat_asset.as_baked().unwrap();
                        let mat_field_reader = read_guard.get_baked_material_asset(baked_mat);

                        let upload = UploadMaterial {
                            metallic: mat_field_reader.metallic(),
                            roughness: mat_field_reader.roughness(),
                            base_color: mat_field_reader.base_color(),
                            emissive: mat_field_reader.emissive(),
                            texture_mapping: mat_field_reader.texture_mapping(),
                            texture_transform: mat_field_reader.texture_transform(),
                        };

                        upload_materials.push(upload);
                    }

                    let tex_refs = read_guard.get_asset_relative_textures(handle)
                        .expect(format!("Failed to get mesh relative textures: {:?}", handle).as_str());

                    for tex_ref in tex_refs.iter() {
                        let _bindless_tex_handle = self.add_bindless_texture(tex_ref.handle());
                    }

                    return self.upload_gpu_mesh_data(packed, colors, uvs, tangents, indices, mat_ids, &upload_materials);
                } else {
                    panic!("Trying to add a non-mesh asset in add_asset_mesh()!");
                }
            }
        };

        MeshHandle { id: u32::MAX }
    }

    pub fn add_mesh_instance(&mut self, transform: Affine3A, handle: MeshHandle) -> MeshInstanceHandle {
        let instance_handle = MeshInstanceHandle(self.mesh_instances.len() as _);
        self.mesh_instances.push(MeshInstance {
            transform,
            handle,
        });
        instance_handle
    }

    fn upload_gpu_mesh_data(&mut self,
        packed: &[PackedVertex], colors: &[[f32; 4]],
        uvs: &[[f32; 2]], tangents: &[[f32; 4]],
        indices: &[u32], mat_ids: &[u32],
        upload_materials: &[UploadMaterial]
    ) -> MeshHandle {
        let curr_global_offset = self.current_draw_data_offset as u32;

        // copy upload
        let mut copy_engine = CopyEngine::new();
        
        let packed_offset  = copy_engine.copy(&packed)   + curr_global_offset;
        let color_offset   = copy_engine.copy(&colors)   + curr_global_offset;
        let uv_offset      = copy_engine.copy(&uvs)      + curr_global_offset;
        let tangent_offset = copy_engine.copy(&tangents) + curr_global_offset;
        let index_offset   = copy_engine.copy(&indices)  + curr_global_offset;
        let mat_id_offset  = copy_engine.copy(&mat_ids)  + curr_global_offset;
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

        MeshHandle {
            id: mesh_id as _,
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
                                // transpose to row-major matrix to be used in shader
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
        for img in self.bindless_images {
            let img = Arc::try_unwrap(img)
                .expect("Failed to clean bindless images!");
            rhi.device.destroy_image(img);
        }

        let draw_data_buffer = Arc::try_unwrap(self.draw_data_buffer).unwrap();
        let mesh_buffer = Arc::try_unwrap(self.mesh_buffer).unwrap();

        rhi.device.destroy_buffer(draw_data_buffer);
        rhi.device.destroy_buffer(mesh_buffer);
        rhi.device.destroy_buffer(self.bindless_tex_sizes_buffer);
    }
}