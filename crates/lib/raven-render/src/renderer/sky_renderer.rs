use std::{sync::Arc};

use ash::vk;

use raven_core::{asset::asset_registry::{AssetHandle, get_runtime_asset_registry}, utility::as_byte_slice_values};
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

    pub fn new_cubemap(rhi: &Rhi, asset: &Arc<AssetHandle>) -> Self {
        unimplemented!("Cubemap");

        Self {
            cubemap: None,
        }
    }

    pub fn new_cubemap_split(rhi: &Rhi, assets: &Vec<Arc<AssetHandle>>) -> Self {
        assert_eq!(assets.len(), 6);
        let cubemap = Self::create_cubemap_split(rhi, assets);

        Self {
            cubemap: Some(cubemap),
        }
    }

    // cubemap split sequence:
    // +X, -X, +Y, -Y, +Z, -Z
    fn create_cubemap_split(rhi: &Rhi, assets: &Vec<Arc<AssetHandle>>) -> Arc<Image> {
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
                                layer: face,
                            })
                            .collect::<Vec<_>>();
                        upload_faces[face as usize] = uploads;
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
