use std::sync::Arc;

use ash::vk;

use glam::Affine3A;
use raven_core::{utility::as_byte_slice_values, asset::asset_registry::AssetHandle};
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable};
use raven_rhi::{Rhi, backend::{ImageDesc, Image, AccessType}};

use crate::{MeshRenderer, IblRenderer, SkyRenderer, MeshRasterScheme, MeshShadingContext, renderer::mesh_renderer::{MeshHandle, InstanceHandle}};

pub struct WorldRenderer {
    render_resolution: [u32; 2],

    sky_renderer: SkyRenderer,
    ibl_renderer: IblRenderer,

    mesh_renderer: MeshRenderer,
}

impl WorldRenderer {
    pub fn new(rhi: &Rhi, render_res: [u32; 2]) -> Self {
        Self {
            render_resolution: render_res,

            sky_renderer: SkyRenderer::new(),
            ibl_renderer: IblRenderer::new(rhi),
            mesh_renderer: MeshRenderer::new(rhi, MeshRasterScheme::Deferred, render_res),
        }
    }

    #[inline]
    pub fn render_resolution(&self) -> [u32; 2] {
        self.render_resolution
    }

    pub fn add_cubemap_split(&mut self, rhi: &Rhi, asset_handles: &[Arc<AssetHandle>]) {
        self.sky_renderer.add_cubemap_split(rhi, asset_handles)
    }

    pub fn add_mesh(&mut self, asset_handle: &Arc<AssetHandle>) -> MeshHandle {
        self.mesh_renderer.add_asset_mesh(asset_handle)
    }

    pub fn add_mesh_instance(&mut self, transform: Affine3A, handle: MeshHandle) -> InstanceHandle {
        self.mesh_renderer.add_mesh_instance(transform, handle)
    }

    pub fn prepare_rg(&mut self, rg: &mut RenderGraphBuilder) -> RgHandle<Image> {
        let main_img_desc: ImageDesc = ImageDesc::new_2d(self.render_resolution, vk::Format::R32G32B32A32_SFLOAT);
        let mut main_img = rg.new_resource(main_img_desc);

        {
            let cubemap = self.sky_renderer.get_cubemap();
            let is_cubemap_exist = cubemap.is_some();

            let cubemap_handle = if let Some(cubemap) = cubemap.as_ref() {
                Some(rg.import(cubemap.clone(), AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
            } else {
                None
            };

            let (convolved_cubemap, prefilter_cubemap, brdf) = if is_cubemap_exist {
                let (convolved, prefilter, brdf) = self.ibl_renderer.convolve_if_needed(rg, cubemap_handle.as_ref().unwrap());
                (Some(convolved), Some(prefilter), Some(brdf))
            } else {
                (None, None, None)
            };
            
            // mesh rasterization
            let shading_context = self.mesh_renderer.prepare_rg(rg);

            // lighting
            match &shading_context {
                MeshShadingContext::GBuffer(gbuffer) => {        
                    let mut pass = rg.add_pass("gbuffer lighting");
                    let pipeline = pass.register_compute_pipeline("defer/defer_lighting.hlsl");

                    let gbuffer_img_ref = pass.read(&gbuffer.packed_gbuffer, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                    let depth_img_ref = pass.read(&gbuffer.depth, AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer);
                    let cubemap_ref = if let Some(cubemap) = cubemap_handle {
                        Some(pass.read(&cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                    } else {
                        None
                    };
                    let convolved_cubemap_ref = if let Some(convolved_cubemap) = convolved_cubemap {
                        Some(pass.read(&convolved_cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                    } else {
                        None
                    };
                    let prefilter_cubemap_ref = if let Some(prefilter_cubemap) = prefilter_cubemap {
                        Some(pass.read(&prefilter_cubemap, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                    } else {
                        None
                    };
                    let brdf_ref = if let Some(brdf) = brdf {
                        Some(pass.read(&brdf, AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer))
                    } else {
                        None
                    };
                    let main_img_ref = pass.write(&mut main_img, AccessType::ComputeShaderWrite);
    
                    let extent = gbuffer.packed_gbuffer.desc().extent;
                    pass.render(move |context| {
                        let mut depth_img_binding = depth_img_ref.bind();
                        depth_img_binding.with_aspect(vk::ImageAspectFlags::DEPTH);

                        // bind pipeline and descriptor set
                        let bound_pipeline = if is_cubemap_exist {
                            context.bind_compute_pipeline(pipeline.into_bindings()
                                .descriptor_set(0, &[
                                    gbuffer_img_ref.bind(),
                                    depth_img_binding,
                                    main_img_ref.bind(),
                                    cubemap_ref.unwrap().bind(),
                                    convolved_cubemap_ref.unwrap().bind(),
                                    prefilter_cubemap_ref.unwrap().bind(),
                                    brdf_ref.unwrap().bind(),
                                ])
                            )?
                        } else {
                            context.bind_compute_pipeline(pipeline.into_bindings()
                                .descriptor_set(0, &[
                                    gbuffer_img_ref.bind(),
                                    depth_img_binding,
                                    main_img_ref.bind()
                                ])
                            )?
                        };

                        let push_constants = [extent[0], extent[1]];
                        bound_pipeline.push_constants(vk::ShaderStageFlags::COMPUTE, 0, as_byte_slice_values(&push_constants));
                        
                        bound_pipeline.dispatch(extent);
    
                        Ok(())
                    });
                },
                _ => unimplemented!(),
            }
        }

        main_img
    }

    pub fn clean(self, rhi: &Rhi) {
        self.mesh_renderer.clean(rhi);

        self.ibl_renderer.clean(rhi);
        self.sky_renderer.clean(rhi);
    }
}