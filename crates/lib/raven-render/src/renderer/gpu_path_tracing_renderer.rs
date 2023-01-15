use std::sync::Arc;

use ash::vk;

use raven_asset::PackedVertex;
use raven_rg::{RenderGraphBuilder, RgHandle, IntoPipelineDescriptorBindings, RenderGraphPassBindable};
use raven_rhi::{
    backend::{
        Device,
        Image,
        RayTracingAccelerationStructure, RayTracingAccelerationScratchBuffer,
        RayTracingBlasBuildDesc, RayTracingGeometry, RayTracingGeometryType, RayTracingSubGeometry,
        RayTracingTlasBuildDesc, RayTracingBlasInstance, AccessType,
        RayTracingPipelineDesc, PipelineShaderDesc, PipelineShaderStage,
    }, 
    Rhi
};

use crate::MeshRenderer;
use super::mesh_renderer::MeshHandle;

const TLAS_PREALLOCATED_BYTES: usize = 32 * 1024 * 1024;

pub struct GpuPathTracingRenderer {
    mesh_blas: Vec<Arc<RayTracingAccelerationStructure>>,
    tlas: Option<Arc<RayTracingAccelerationStructure>>,
    tlas_scratch_buffer: RayTracingAccelerationScratchBuffer,

    device: Arc<Device>,
}

impl GpuPathTracingRenderer {
    pub fn new(rhi: &Rhi) -> Self {
        let tlas_scratch_buffer = rhi.device
            .create_ray_tracing_acceleration_scratch_buffer()
            .expect("Faield to prepare ray tracing tlas scratch buffer!");

        // build a empty tlas, and update later
        let tlas = Some(Arc::new(rhi.device.build_tlas(
            RayTracingTlasBuildDesc::empty(TLAS_PREALLOCATED_BYTES), 
            &tlas_scratch_buffer
        ).expect("Failed to prepare ray tracing tlas!")));

        Self {
            mesh_blas: Default::default(),
            tlas,
            tlas_scratch_buffer,

            device: rhi.device.clone(),
        }
    }

    pub fn add_mesh(&mut self, handle: MeshHandle, mesh_renderer: &MeshRenderer) {
        let uploaded_mesh = mesh_renderer.get_uploaded_mesh_data(handle);

        // TODO: builder mode
        let blas = self.device.build_blas(RayTracingBlasBuildDesc {
            geometries: vec![
                RayTracingGeometry {
                    geo_type: RayTracingGeometryType::Triangle,
                    /// Referenced vertex buffer address.
                    vb_address: uploaded_mesh.vertex_packed_address,
                    /// Referenced index buffer address.
                    ib_address: uploaded_mesh.index_buffer_address,
                    vertex_format: vk::Format::R32G32B32_SFLOAT,
                    vertex_stride: std::mem::size_of::<PackedVertex>(),
                    sub_geometries: vec![RayTracingSubGeometry {
                        index_count: uploaded_mesh.index_count as usize,
                        index_offset: 0,
                        max_vertex: uploaded_mesh.max_vertex,
                    }],
                }
            ]
        })
        .expect("Failed to create new mesh blas!");

        self.mesh_blas.push(Arc::new(blas));
    }

    pub fn update_tlas(&mut self, rg: &mut RenderGraphBuilder, mesh_renderer: &MeshRenderer) -> RgHandle<RayTracingAccelerationStructure> {
        let mesh_instances = mesh_renderer.get_mesh_instances();

        let blas_instances = mesh_instances.iter()
            .map(|inst| RayTracingBlasInstance {
                blas: self.mesh_blas[inst.handle.id as usize].clone(),
                affine_xform: inst.transform,
                mesh_index: inst.handle.id as u32,
            })
            .collect::<Vec<_>>();

        let mut tlas = rg.import(self.tlas.as_ref().unwrap().clone(), AccessType::AnyShaderReadOther);

        {
            let mut pass = rg.add_pass("update tlas");
            let tlas_ref = pass.write(&mut tlas, AccessType::TransferWrite);

            // although here we use clone(), but it reference to the same buffer
            let tlas_scratch_buffer = self.tlas_scratch_buffer.clone();

            pass.render(move |ctx| {
                let registry = &mut ctx.registry;

                let instance_buffer_addr = registry
                    .execution_params
                    .device
                    .fill_ray_tracing_tlas_instance_buffer(
                        registry.global_dynamic_buffer,
                        blas_instances.as_slice()
                    );

                let tlas = registry.get_acceleration_structure(tlas_ref);
            
                registry.execution_params.device.update_tlas(
                    ctx.cb.raw,
                    instance_buffer_addr,
                    blas_instances.len(),
                    tlas,
                    &tlas_scratch_buffer,
                );

                Ok(())
            });
        }

        tlas
    }

    pub fn path_tracing_accum(
        &mut self,
        rg: &mut RenderGraphBuilder,
        tlas: &RgHandle<RayTracingAccelerationStructure>,
        accum_img: &mut RgHandle<Image>,
        env_map: &RgHandle<Image>,
        bindless_descriptor_set: vk::DescriptorSet,
    ) {
        let mut pass = rg.add_pass("path tracing");
        let pipeline = pass.register_ray_tracing_pipeline(&[
            PipelineShaderDesc::builder()
                .source("path_tracing/accumulate/path_trace_accum.rgen.hlsl")
                .stage(PipelineShaderStage::RayGen)
                .build().unwrap(),
            PipelineShaderDesc::builder()
                .source("path_tracing/accumulate/ray_tracing_gbuffer.rmiss.hlsl")
                .stage(PipelineShaderStage::RayMiss)
                .build().unwrap(),
            PipelineShaderDesc::builder()
                .source("path_tracing/accumulate/shadow.rmiss.hlsl")
                .stage(PipelineShaderStage::RayMiss)
                .build().unwrap(),
            PipelineShaderDesc::builder()
                .source("path_tracing/accumulate/path_trace_accum.rchit.hlsl")
                .stage(PipelineShaderStage::RayClosestHit)
                .build().unwrap(),
        ], RayTracingPipelineDesc::builder()
            .max_ray_recursive_depth(1)
            .build().unwrap()
        );

        let tlas_ref = pass.read(tlas, AccessType::RayTracingShaderReadAccelerationStructure);
        let accum_ref = pass.write(accum_img, AccessType::AnyShaderWrite);
        let env_ref = pass.read(env_map, AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer);

        let accum_img_extent = accum_img.desc().extent;

        pass.render(move |ctx| {
            let bound_pipeline = ctx.bind_ray_tracing_pipeline(
                pipeline.into_bindings()
                    .descriptor_set(0, &[
                        tlas_ref.bind(),
                        accum_ref.bind(),
                        env_ref.bind(),
                    ])
                    .raw_descriptor_set(1, bindless_descriptor_set)
            )?; 

            bound_pipeline.trace_rays(accum_img_extent);

            Ok(())
        });
    }

    pub fn clean(self, rhi: &Rhi) {
        for blas in self.mesh_blas {
            let blas = Arc::try_unwrap(blas)
                .expect("Failed to release blas, someone is still using it!");

            rhi.device.destroy_acceleration_structure(blas);
        }

        if let Some(tlas) = self.tlas {
            let tlas = Arc::try_unwrap(tlas)
                .expect("Failed to release tlas, someone is still using it!");

            rhi.device.destroy_acceleration_structure(tlas);
        }

        rhi.device.destroy_ray_tracing_scratch_buffer(self.tlas_scratch_buffer);
    }
}