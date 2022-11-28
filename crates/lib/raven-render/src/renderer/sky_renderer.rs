use std::{sync::Arc, f32::consts::PI};

use ash::vk;

use glam::Vec3;
use raven_core::{asset::{asset_registry::{AssetHandle, get_runtime_asset_registry}, AssetType, VecArrayQueryParam}, math::{SHBasis9, from_rgb8_to_color}};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable, RenderGraphPassBinding};
use raven_rhi::{backend::{Image, ImageDesc, ImageSubresource, AccessType}, Rhi};

use crate::MeshShadingContext;

pub struct SkyRenderer {
    cubemap: Option<Arc<Image>>,
}

impl SkyRenderer {
    pub fn new() -> Self {
        Self {
            cubemap: None,
        }
    }

    // pub fn new_cubemap(rhi: &Rhi, asset: &Arc<AssetHandle>) -> Self {
    //     unimplemented!("Cubemap");

    //     Self {
    //         cubemap: None,
    //     }
    // }

    pub fn new_cubemap_split(rhi: &Rhi, assets: &[Arc<AssetHandle>]) -> Self {
        assert_eq!(assets.len(), 6);
        let cubemap = Self::create_cubemap_split(rhi, assets);

        Self {
            cubemap: Some(cubemap),
        }
    }

    pub fn add_cubemap_split(&mut self, rhi: &Rhi, assets: &[Arc<AssetHandle>]) {
        let cubemap = Self::create_cubemap_split(rhi, assets);
        // override the old one
        // TODO: delete the old one
        self.cubemap = Some(cubemap);
    }

    // cubemap split sequence:
    // +X, -X, +Y, -Y, +Z, -Z
    fn create_cubemap_split(rhi: &Rhi, assets: &[Arc<AssetHandle>]) -> Arc<Image> {
        let device = &rhi.device;
        let asset_registry = get_runtime_asset_registry(); 

        let mut extent = [0, 0, 0];
        let cubemap = {
            let read_guard = asset_registry.read();
            let mut upload_faces: [Vec<ImageSubresource<'_>>; 6] = Default::default();

            let mut face = 0;
            for asset in assets {
                if let Some(asset) = read_guard.get_asset(asset) {
                    if let Some(tex) = asset.as_texture() {
                        if face != 0 {
                            assert_eq!(extent[0], tex.extent[0]);
                            assert_eq!(extent[1], tex.extent[1]);
                            assert_eq!(extent[2], tex.extent[2]);
                        }
                        // each face's extent must be the same
                        extent = tex.extent;
                        assert_eq!(extent[0], extent[1]); // width height must be the same

                        let uploads = tex.lod_groups.iter()
                            .map(|mip| ImageSubresource {
                                data: mip.as_slice(),
                                // TODO: no hardcode
                                row_pitch_in_bytes: extent[0] * 4,
                                base_layer: face,
                            })
                            .collect::<Vec<_>>();
                        upload_faces[face as usize] = uploads;
                    }
                    else if let Some(baked) = asset.as_baked() {
                        match baked.origin_asset_type() {
                            AssetType::Texture => {
                                let field_reader = read_guard.get_baked_texture_asset(baked);
                                let tex_extent = field_reader.extent();

                                if face != 0 {
                                    assert_eq!(extent[0], tex_extent[0]);
                                    assert_eq!(extent[1], tex_extent[1]);
                                    assert_eq!(extent[2], tex_extent[2]);
                                }
                                // each face's extent must be the same
                                extent = tex_extent;
                                assert_eq!(extent[0], extent[1]); // width height must be the same

                                let mips_len = field_reader.lod_groups(VecArrayQueryParam::length()).length();

                                let uploads = (0..mips_len).into_iter()
                                    .map(|idx| {
                                        let mip = field_reader.lod_groups(VecArrayQueryParam::index(idx)).array();
                                    
                                        ImageSubresource {
                                            data: mip,
                                            // TODO: no hardcode
                                            row_pitch_in_bytes: extent[0] * 4,
                                            base_layer: face,
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                upload_faces[face as usize] = uploads;
                            }
                            _ => unreachable!()
                        }
                    }
                }

                face += 1;
            }

            let cubemap = device.create_image(
                ImageDesc::new_cube(extent[0], vk::Format::R8G8B8A8_UNORM)
                    .usage_flags(vk::ImageUsageFlags::SAMPLED)
                    .mipmap_level(upload_faces[0].len() as _),
                Some(vec![])
            ).expect("Failed to create sky renderer cubemap texture!");

            device.upload_image_data(&cubemap, &upload_faces,
                AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer)
                .expect("Failed to upload image data during cubemap building!");
            
            cubemap
        };
        
        Arc::new(cubemap)
    }

    pub fn precalculate_sh_split(&self, assets: &[Arc<AssetHandle>]) -> [SHBasis9; 3] {
        let asset_registry = get_runtime_asset_registry(); 

        {
            let read_guard = asset_registry.read();
            
            let mut red_basis = SHBasis9::zero();
            let mut green_basis = SHBasis9::zero();
            let mut blue_basis = SHBasis9::zero();

            let mut extent = [0; 3];

            for (face, asset) in assets.iter().enumerate() {
                if let Some(asset) = read_guard.get_asset(asset) {
                    if let Some(_tex) = asset.as_texture() {
                        unimplemented!()
                    } else if let Some(baked) = asset.as_baked() {
                        match baked.origin_asset_type() {
                            AssetType::Texture => {
                                let field_reader = read_guard.get_baked_texture_asset(baked);
                                let tex_extent = field_reader.extent();
                                assert_eq!(tex_extent[0], tex_extent[1]);
                                extent = tex_extent;

                                for y in 0..tex_extent[1] {
                                    for x in 0..tex_extent[0] {
                                        let dir = Self::get_direction(face, x, y, tex_extent[0], tex_extent[1]);

                                        let basis = SHBasis9::from_direction_cartesian(dir);

                                        // TODO: use a sample callback function
                                        let datas = field_reader.lod_groups(VecArrayQueryParam::Index(0)).array();
                                        let base_offset = ((y * tex_extent[0] + x) * 4) as usize;

                                        let sample = from_rgb8_to_color(datas[base_offset], datas[base_offset + 1], datas[base_offset + 2]);
                                    
                                        red_basis = red_basis.add(&basis.mul_scaler(sample.x));
                                        green_basis = green_basis.add(&basis.mul_scaler(sample.y));
                                        blue_basis = blue_basis.add(&basis.mul_scaler(sample.z));
                                    }
                                }
                                
                            }
                            _ => unreachable!()
                        }
                    }
                }
            }

            let factor = 4.0 * PI / (extent[0] as f32 * extent[1] as f32 * 6.0);

            red_basis = red_basis.mul_scaler(factor);
            green_basis = green_basis.mul_scaler(factor);
            blue_basis = blue_basis.mul_scaler(factor);

            [
                red_basis,
                green_basis,
                blue_basis,
            ]
        }
    }

    fn get_direction(face: usize, x: u32, y: u32, res_x: u32, res_y: u32) -> Vec3 {
        let mut dir = Vec3::from_array([
            x as f32 / (res_x - 1) as f32 * 2.0 - 1.0,
            y as f32 / (res_y - 1) as f32 * 2.0 - 1.0,
            -1.0
        ]);
        dir = dir.normalize();

        match face {
            0 => { // +X
               let temp = dir[0];
               dir[0] = -dir[2];
               dir[2] = temp;
            }
            1 => { // -X
                let temp = dir[0];
                dir[0] = dir[2];
                dir[2] = -temp;
            }
            2 => { // +Y
                let temp = dir[1];
                dir[1] = -dir[2];
                dir[2] = temp;
            }
            3 => { // -Y
                let temp = dir[1];
                dir[1] = dir[2];
                dir[2] = -temp;
            }
            4 => { // +Z
                dir[0] *= -1.0;
                dir[2] *= -1.0;
            }
            5 => { // -Z
            }
            _ => panic!("Cubemap face index out of range! {}", face)
        };

        dir
    }

    #[inline]
    pub fn get_cubemap(&self) -> &Option<Arc<Image>> {
        &self.cubemap
    }

    pub fn prepare_rg(&self, rg: &mut RenderGraphBuilder, shading_context: &MeshShadingContext, output_img: &mut RgHandle<Image>) {
        if let Some(cubemap) = self.cubemap.clone() {
            let cubemap = rg.import(cubemap, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
            let depth = match shading_context {
                MeshShadingContext::GBuffer(gbuffer) => {
                    &gbuffer.depth
                }
                _ => unimplemented!()
            };

            let extent = output_img.desc().extent;
            {
                let mut pass = rg.add_pass("sky render");
                let pipeline = pass.register_compute_pipeline("sky_render.hlsl");

                let depth_ref = pass.read(depth, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let cubemap_ref = pass.read(&cubemap, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                let output_ref = pass.write(output_img, AccessType::ComputeShaderWrite);

                pass.render(move |ctx| {
                    let push_extent = [extent[0], extent[1]];
                    let extent_offset = ctx.global_dynamic_buffer().push(&push_extent);

                    let mut depth_binding = depth_ref.bind();
                    depth_binding.with_aspect(vk::ImageAspectFlags::DEPTH);

                    let bound_pipeline = ctx.bind_compute_pipeline(pipeline.into_bindings()
                        .descriptor_set(0, &[
                            depth_binding,
                            cubemap_ref.bind(),
                            output_ref.bind(),
                            RenderGraphPassBinding::DynamicBuffer(extent_offset),
                        ])
                    )?;

                    bound_pipeline.dispatch(extent);

                    Ok(())
                });
            }
        }
    }

    pub fn clean(self, rhi: &Rhi) {
        if let Some(cubemap) = self.cubemap {
            let cubemap = Arc::try_unwrap(cubemap).unwrap_or_else(|_| panic!("Failed to release cubemap, someone is still using it!"));

            rhi.device.destroy_image(cubemap);
        }
    }
}
