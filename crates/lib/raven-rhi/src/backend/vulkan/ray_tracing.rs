use std::{sync::Arc, cell::RefCell};

use ash::vk;
use parking_lot::Mutex;

use raven_math::{self, Affine3A};

use crate::{dynamic_buffer::DynamicBuffer};

use super::{Device, RhiError, Buffer, pipeline::{RayTracingShaderBindingTable, RayTracingShaderBindingTableDesc}};

#[derive(Debug, Clone)]
pub enum RayTracingGeometryType {
    Triangle = 0,
    AABB = 1,
}

#[derive(Copy, Clone, Debug)]
pub struct RayTracingSubGeometry {
    /// Index count within this sub geometry.
    pub index_count: usize,
    /// The first used index offset of the index buffer (passed by vk::DeviceAddress)
    pub index_offset: usize,
    /// The highest index of a vertex which will be addressed by a build command using this BLAS
    pub max_vertex: u32,
}

#[derive(Clone, Debug)]
pub struct RayTracingGeometry {
    /// Geometry type used in acceleration structures.
    pub geo_type: RayTracingGeometryType,
    /// Referenced vertex buffer address.
    pub vb_address: vk::DeviceAddress,
    /// Referenced index buffer address.
    pub ib_address: vk::DeviceAddress,
    pub vertex_format: vk::Format,
    pub vertex_stride: usize,
    pub sub_geometries: Vec<RayTracingSubGeometry>,
}

/// A ray tracing BLAS (Bottom-Level-Acceleration-Structure).
/// One BLAS can contain multiple geometry datas by single affine matrix.
/// The less BLAS, the more efficient ray tracing will be.
/// You can batch multiple static geometry into one single BLAS.
#[derive(Debug, Clone)]
pub struct RayTracingBlasBuildDesc {
    pub geometries: Vec<RayTracingGeometry>,
}

/// Each instance store a blas reference pointer and the transform of the blas.
#[derive(Clone)]
pub struct RayTracingBlasInstance {
    pub blas: Arc<RayTracingAccelerationStructure>,
    pub affine_xform: Affine3A,
    pub mesh_index: u32,
}

pub struct RayTracingTlasBuildDesc {
    pub instances: Vec<RayTracingBlasInstance>,
    pub preallocate_bytes: usize,
}

impl RayTracingTlasBuildDesc {
    pub fn empty(preallocate_bytes: usize) -> Self {
        Self {
            instances: Vec::new(),
            preallocate_bytes
        }
    }
}

#[derive(Debug)]
pub struct RayTracingAccelerationStructure {
    pub raw: vk::AccelerationStructureKHR,
    /// The buffer which contained the acceleration structure data.
    backing_buffer: Buffer,
    // TODO: this must be removed!
    init_instance_buffer: RefCell<Option<Buffer>>,
}

#[derive(Clone)]
pub struct RayTracingAccelerationScratchBuffer {
    buffer: Arc<Mutex<Buffer>>,
}

/// Same structure of VkAccelerationStructureInstanceKHR
#[repr(C)]
#[derive(Clone, Debug, Copy)]
struct RayTracingGeometryInstance {
    transform: [f32; 12], // affine transform matrix (row major)
    /// Store instance id in low 24 bits, (maximum up to 2^24 instances).
    /// Store mask id in high 8 bits
    instance_id_and_mask: u32,
    /// Store sbt offset in low 24 bits.
    /// Store flags in high 8 bits
    instance_sbt_offset_and_flags: u32, // sbt stands for shader binding table
    blas_address: vk::DeviceAddress,
}

impl RayTracingGeometryInstance {
    fn new(
        transform: [f32; 12],
        instance_id: u32,
        instance_mask: u32,
        sbt_offset: u32,
        flags: vk::GeometryInstanceFlagsKHR,
        blas_address: vk::DeviceAddress,
    ) -> Self {
        let mut inst = RayTracingGeometryInstance {
            transform,
            instance_id_and_mask: 0,
            instance_sbt_offset_and_flags: 0,
            blas_address,
        };

        inst.set_instance_id(instance_id);
        inst.set_mask(instance_mask);
        inst.set_sbt_offset(sbt_offset);
        inst.set_flags(flags);

        inst
    }

    fn set_instance_id(&mut self, instance_id: u32) {
        let id = instance_id & 0x00ffffff;
        self.instance_id_and_mask |= id;
    }

    fn set_mask(&mut self, mask: u32) {
        let mask = mask as u32;
        self.instance_id_and_mask |= mask << 24;
    }

    fn set_sbt_offset(&mut self, sbt_offset: u32) {
        let offset = sbt_offset & 0x00ffffff;
        self.instance_sbt_offset_and_flags |= offset;
    }

    fn set_flags(&mut self, flags: vk::GeometryInstanceFlagsKHR) {
        let flags = flags.as_raw() as u32;
        self.instance_sbt_offset_and_flags |= flags << 24;
    }
}

impl Device {
    pub fn create_ray_tracing_acceleration_scratch_buffer(
        &self
    ) -> anyhow::Result<RayTracingAccelerationScratchBuffer, RhiError> {
        const RAY_TRACING_TLAS_SCRATCH_BUFFER_SIZE: usize = 256 * 1024; // 256k

        let buffer = self.create_buffer(
            super::buffer::BufferDesc::new_gpu_only(
                RAY_TRACING_TLAS_SCRATCH_BUFFER_SIZE,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            )
            .alignment(self.ray_tracing_extensions.acceleration_structure_props.min_acceleration_structure_scratch_offset_alignment as _),
            "tlas scratch buffer",
        )?;

        Ok(RayTracingAccelerationScratchBuffer {
            buffer: Arc::new(Mutex::new(buffer)),
        })
    }

    pub fn build_blas(
        &self,
        blas_desc: RayTracingBlasBuildDesc
    ) -> anyhow::Result<RayTracingAccelerationStructure, RhiError> {
        const BUILD_AS_TYPE: vk::AccelerationStructureTypeKHR = vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL;

        // 1. fill the VkAccelerationStructureGeometryKHR 
        let geometries: anyhow::Result<Vec<vk::AccelerationStructureGeometryKHR>, RhiError> = blas_desc.geometries.iter()
            .map(|geo| -> anyhow::Result<vk::AccelerationStructureGeometryKHR, RhiError> {
                // TODO: combine sub-geometries
                let sub_geometry = geo.sub_geometries[0];

                let as_geo = vk::AccelerationStructureGeometryKHR::builder()
                    .geometry_type(match geo.geo_type {
                        RayTracingGeometryType::Triangle => vk::GeometryTypeKHR::TRIANGLES,
                        RayTracingGeometryType::AABB => vk::GeometryTypeKHR::AABBS,
                    })
                    .geometry(vk::AccelerationStructureGeometryDataKHR {
                        triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::builder()
                            .vertex_data(vk::DeviceOrHostAddressConstKHR {
                                device_address: geo.vb_address,
                            })
                            .vertex_stride(geo.vertex_stride as _)
                            .max_vertex(sub_geometry.max_vertex)
                            .vertex_format(geo.vertex_format)
                            .index_data(vk::DeviceOrHostAddressConstKHR {
                                device_address: geo.ib_address,
                            })
                            .index_type(vk::IndexType::UINT32) // TODO
                            .build(),
                    })
                    .flags(vk::GeometryFlagsKHR::OPAQUE)
                    .build();

                Ok(as_geo)
            })
            .collect();
        let geometries = geometries?;

        // 2. specified VkAccelerationStructureBuildRangeInfoKHR and each max_prims
        let (build_range_infos, max_primitive_counts): (Vec<vk::AccelerationStructureBuildRangeInfoKHR>, Vec<_>) = blas_desc.geometries.iter()
            .map(|desc| {
                let primitive_count = desc.sub_geometries[0].index_count as u32 / 3;

                (vk::AccelerationStructureBuildRangeInfoKHR::builder()
                    // TODO: combine sub-geometries
                    // first vertex is 0 and no offset by default
                    .primitive_count(primitive_count)
                    .build(),
                primitive_count)
            })
            .unzip();

        // 3. build blas geometry info
        let geometry_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(BUILD_AS_TYPE)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(geometries.as_slice())
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .build();

        self.build_acceleration_structure(
            BUILD_AS_TYPE,
            geometry_build_info,
            &build_range_infos,
            &max_primitive_counts,
            0,
            None,

            None,
        )
    }

    pub fn build_tlas(
        &self,
        tlas_desc: RayTracingTlasBuildDesc,
        scratch_buffer: &RayTracingAccelerationScratchBuffer,
    ) -> anyhow::Result<RayTracingAccelerationStructure, RhiError> {
        const BUILD_AS_TYPE: vk::AccelerationStructureTypeKHR = vk::AccelerationStructureTypeKHR::TOP_LEVEL;

        // 1. create instances buffer
        let geo_instances = tlas_desc.instances.iter()
            .map(|instance| {
                // row major
                let transform_mat = [
                    instance.affine_xform.x_axis.x,
                    instance.affine_xform.y_axis.x,
                    instance.affine_xform.z_axis.x,
                    instance.affine_xform.translation.x,
                    instance.affine_xform.x_axis.y,
                    instance.affine_xform.y_axis.y,
                    instance.affine_xform.z_axis.y,
                    instance.affine_xform.translation.y,
                    instance.affine_xform.x_axis.z,
                    instance.affine_xform.y_axis.z,
                    instance.affine_xform.z_axis.z,
                    instance.affine_xform.translation.z,
                ];

                let blas_address = unsafe {
                    self.ray_tracing_extensions.acceleration_structure_khr.get_acceleration_structure_device_address(
                        &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
                            .acceleration_structure(instance.blas.raw)
                            .build(),
                    )
                };

                RayTracingGeometryInstance::new(
                    transform_mat,
                    instance.mesh_index, // each mesh is a instance
                    0xff, // full mask by default
                    0, // offset will be filled later
                    vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
                    blas_address
                )
            })
            .collect::<Vec<_>>();

        let instance_buffer_size = std::mem::size_of::<RayTracingGeometryInstance>() * geo_instances.len().max(1);
        let instance_buffer = self.create_buffer_init(
            super::buffer::BufferDesc::new_gpu_only(
                instance_buffer_size,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
            ),
            "Acceleration tlas instance buffer",
            &geo_instances
        )?;
        
        let instance_buffer_address = instance_buffer.device_address(self);

        // 2. fill instances geometry info
        let geometry_info = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
                    .data(vk::DeviceOrHostAddressConstKHR {
                        device_address: instance_buffer_address,
                    })
                    .build(),
            })
            .build();

        // 3. specified build range
        let build_range_infos = vec![vk::AccelerationStructureBuildRangeInfoKHR::builder()
            .primitive_count(geo_instances.len() as _)
            .build()];

        // 4. fill geometry build info
        let geometry_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(BUILD_AS_TYPE)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(std::slice::from_ref(&geometry_info))
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .build();

        let max_primitive_counts = [geo_instances.len() as u32]; // have only one tlas
        
        self.build_acceleration_structure(
            BUILD_AS_TYPE,
            geometry_build_info,
            &build_range_infos,
            &max_primitive_counts,
            tlas_desc.preallocate_bytes,
            Some(scratch_buffer),

            Some(instance_buffer),
        )
    }

    /// # Note
    /// 
    /// We may want to reuse the scratch buffer here when build tlas.
    /// Use Option here can pass optional scratch buffer in it.
    fn build_acceleration_structure(
        &self,
        ty: vk::AccelerationStructureTypeKHR,
        mut geometry_build_info: vk::AccelerationStructureBuildGeometryInfoKHR,
        build_range_infos: &[vk::AccelerationStructureBuildRangeInfoKHR],
        max_primitive_counts: &[u32],
        preallocate_bytes: usize,
        scratch_buffer: Option<&RayTracingAccelerationScratchBuffer>,
        backing_instance_buffer: Option<Buffer>,
    ) -> anyhow::Result<RayTracingAccelerationStructure, RhiError> {
        // finding sizes to create acceleration structures and scratch
        // TODO: this function will return the sizes in the worst case, to optimize this,
        // use VK_QUERY_TYPE_ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR to query the compact size.
        // This could save a lot of memory usage.
        // Use VkQueryPool to get the compact size of each acceleration structure and copy it.
        let memory_requirements = unsafe {
            self.ray_tracing_extensions.acceleration_structure_khr
                .get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_build_info,
                    max_primitive_counts,
                )
        };

        glog::info!(
            "Build acceleration structure size: {}, scratch size (build, update): ({}, {})",
            memory_requirements.acceleration_structure_size,
            memory_requirements.build_scratch_size,
            memory_requirements.update_scratch_size
        );

        // check if the preallocate buffer can store the accel struct data
        let backing_buffer_size = preallocate_bytes.max(memory_requirements.acceleration_structure_size as usize);

        let backing_buffer = self.create_buffer(
            super::buffer::BufferDesc::new_gpu_only(
                backing_buffer_size,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR |
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            ),
            "Acceleration structure backing buffer",
        )?;

        // build as create info
        let accel_info = vk::AccelerationStructureCreateInfoKHR::builder()
            .ty(ty)
            .buffer(backing_buffer.raw)
            .size(backing_buffer_size as u64)
            .build();

        let mut temp_scratch_buffer = None;
        let mut scratch_buffer_locked;

        // create scratch buffer for building
        let scratch_buffer = if let Some(scratch_buffer) = scratch_buffer {
            // used scratch buffer outside
            scratch_buffer_locked = scratch_buffer.buffer.lock();
            &mut *scratch_buffer_locked
        } else {
            temp_scratch_buffer = Some(
                self.create_buffer(
                    super::buffer::BufferDesc::new_gpu_only(
                        memory_requirements.build_scratch_size as usize,
                        vk::BufferUsageFlags::STORAGE_BUFFER |
                        vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                    )
                    .alignment(self.ray_tracing_extensions.acceleration_structure_props.min_acceleration_structure_scratch_offset_alignment as _),
                    "Acceleration structure scratch buffer",
                )?,
            );

            temp_scratch_buffer.as_mut().unwrap()
        };

        let accel_raw = unsafe { self.ray_tracing_extensions.acceleration_structure_khr
            .create_acceleration_structure(&accel_info, None)
        }?;

        assert!(
            memory_requirements.build_scratch_size as usize <= scratch_buffer.desc.size,
            "Invalid scratch buffer!"
        );

        geometry_build_info.dst_acceleration_structure = accel_raw;
        geometry_build_info.scratch_data = vk::DeviceOrHostAddressKHR { device_address: scratch_buffer.device_address(self) };

        unsafe {
            // build and wait device idle
            self.with_setup_commands(|cb| {
                self.ray_tracing_extensions.acceleration_structure_khr
                    .cmd_build_acceleration_structures(
                        cb,
                        std::slice::from_ref(&geometry_build_info),
                        std::slice::from_ref(&build_range_infos),
                    );
                
                // wait for building complete
                self.raw.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::DependencyFlags::empty(),
                    &[vk::MemoryBarrier::builder()
                        .src_access_mask(
                            vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR |
                            vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                        )
                        .dst_access_mask(
                            vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR |
                            vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                        )
                        .build()],
                    &[],
                    &[],
                );
            })?;
        }

        // TODO: add query pool to find the real compact size of the acceleration structure
        // use vkCmdWriteAccelerationStructuresPropertiesKHR

        if let Some(temp_scratch_buffer) = temp_scratch_buffer {
            self.destroy_buffer(temp_scratch_buffer);
        }

        Ok(RayTracingAccelerationStructure { 
            raw: accel_raw, 
            backing_buffer,

            init_instance_buffer: RefCell::new(backing_instance_buffer),
        })
    }
    
    pub fn create_ray_tracing_shader_binding_table(
        &self,
        desc: RayTracingShaderBindingTableDesc,
        pipeline_raw: vk::Pipeline
    ) -> anyhow::Result<RayTracingShaderBindingTable, RhiError> {
        // get shader group handle size
        let shader_group_handle_size = self.ray_tracing_extensions
            .ray_tracing_props
            .shader_group_handle_size as usize;
        let shader_group_handle_size_aligned = raven_math::min_value_align_to(
            shader_group_handle_size,
            self.ray_tracing_extensions.ray_tracing_props.shader_group_handle_alignment as usize
        );

        let group_count = (desc.raygen_entry_count + desc.miss_entry_count + desc.hit_entry_count) as usize;
        let group_handles_size = (shader_group_handle_size * group_count) as usize;

        // fetch shader binding handles from the created pipeline object
        let group_handles: Vec<u8> = unsafe {
            self.ray_tracing_extensions
                .ray_tracing_pipeline_khr
                .get_ray_tracing_shader_group_handles(
                    pipeline_raw,
                    0, // now only have one hit group
                    group_count as _,
                    group_handles_size,
                )?
        };

        let create_shader_binding_table_func = |
            entry_offset: u32, 
            entry_count: u32
        | -> anyhow::Result<Option<Buffer>, RhiError> {
            // no entry means no backing buffer
            if entry_count == 0 {
                return Ok(None);
            }
            
            // reserve byte update memory
            // no memory alignment requirement here
            let mut shader_binding_table_bytes = vec![0u8; raven_math::min_value_align_to(
                (entry_count as usize * shader_group_handle_size_aligned) as usize,
                self.ray_tracing_extensions.ray_tracing_props.shader_group_base_alignment as usize
            )];

            for dst in 0..(entry_count as usize) {
                let src = dst + entry_offset as usize;

                let src_handle_byte_range = (src * shader_group_handle_size)..(src * shader_group_handle_size + shader_group_handle_size);
                let dst_handle_byte_range = (dst * shader_group_handle_size_aligned)..(dst * shader_group_handle_size_aligned + shader_group_handle_size);

                shader_binding_table_bytes[dst_handle_byte_range].copy_from_slice(&group_handles[src_handle_byte_range]);
            }

            let sbt_buffer = self.create_buffer_init(
                super::buffer::BufferDesc::new_gpu_only(
                    shader_binding_table_bytes.len(),
                    vk::BufferUsageFlags::TRANSFER_SRC | 
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
                    vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR,
                ),
                "ray tracing sbt sub-buffer",
                &shader_binding_table_bytes
            )?;

            Ok(Some(sbt_buffer))
        };

        let mut entry_offset = 0;
        let ray_gen_shader_sbt_buffer = create_shader_binding_table_func(entry_offset, desc.raygen_entry_count)?;
        entry_offset += desc.raygen_entry_count;

        let ray_miss_shader_sbt_buffer = create_shader_binding_table_func(entry_offset, desc.miss_entry_count)?;
        entry_offset += desc.miss_entry_count;

        let ray_hit_shader_sbt_buffer = create_shader_binding_table_func(entry_offset, desc.hit_entry_count)?;

        let raygen_shader_binding_table_stride = raven_math::min_value_align_to(
            (desc.raygen_entry_count as usize * shader_group_handle_size_aligned) as usize,
            self.ray_tracing_extensions.ray_tracing_props.shader_group_base_alignment as usize
        ) as vk::DeviceSize;

        Ok(RayTracingShaderBindingTable {
            raygen_shader_binding_table: vk::StridedDeviceAddressRegionKHR::builder()
                .device_address(ray_gen_shader_sbt_buffer.as_ref().map(|buf| buf.device_address(self)).unwrap_or(0))
                // The size member of pRayGenShaderBindingTable must be equal to its stride member
                .stride(raygen_shader_binding_table_stride)
                .size(raygen_shader_binding_table_stride)
                .build(),
            raygen_shader_binding_table_buffer: ray_gen_shader_sbt_buffer,

            miss_shader_binding_table: vk::StridedDeviceAddressRegionKHR::builder()
                .device_address(ray_miss_shader_sbt_buffer.as_ref().map(|buf| buf.device_address(self)).unwrap_or(0))
                .stride(shader_group_handle_size_aligned as u64)
                .size(raven_math::min_value_align_to(
                    (desc.miss_entry_count as usize * shader_group_handle_size_aligned) as usize,
                    self.ray_tracing_extensions.ray_tracing_props.shader_group_base_alignment as usize
                ) as vk::DeviceSize)
                .build(),
            miss_shader_binding_table_buffer: ray_miss_shader_sbt_buffer,

            hit_shader_binding_table: vk::StridedDeviceAddressRegionKHR::builder()
                .device_address(ray_hit_shader_sbt_buffer.as_ref().map(|buf| buf.device_address(self)).unwrap_or(0))
                .stride(shader_group_handle_size_aligned as u64)
                .size(raven_math::min_value_align_to(
                    (desc.hit_entry_count as usize * shader_group_handle_size_aligned) as usize,
                    self.ray_tracing_extensions.ray_tracing_props.shader_group_base_alignment as usize
                ) as vk::DeviceSize)
                .build(),
            hit_shader_binding_table_buffer: ray_hit_shader_sbt_buffer,

            callable_shader_binding_table: vk::StridedDeviceAddressRegionKHR::default(),
            callable_shader_binding_table_buffer: None,
        })
    }

    pub fn destroy_ray_tracing_shader_binding_table(
        &self,
        sbt: RayTracingShaderBindingTable
    ) {
        if let Some(buf) = sbt.raygen_shader_binding_table_buffer {
            self.destroy_buffer(buf);
        }
        if let Some(buf) = sbt.miss_shader_binding_table_buffer {
            self.destroy_buffer(buf);
        }
        if let Some(buf) = sbt.hit_shader_binding_table_buffer {
            self.destroy_buffer(buf);
        }
        if let Some(buf) = sbt.callable_shader_binding_table_buffer {
            self.destroy_buffer(buf);
        }
    }

    pub fn update_tlas(
        &self,
        cb_raw: vk::CommandBuffer,
        instance_buffer_address: vk::DeviceAddress,
        new_instance_count: usize,
        update_tlas: &RayTracingAccelerationStructure,
        scratch_buffer: &RayTracingAccelerationScratchBuffer,
    ) {
        if update_tlas.init_instance_buffer.borrow().is_some() {
            let init_instance_buffer = update_tlas.init_instance_buffer.take().unwrap();
            self.defer_release(init_instance_buffer);
        }

        // pretty much the same as build_tlas()
        // we do not care instance buffer here.
        const BUILD_AS_TYPE: vk::AccelerationStructureTypeKHR = vk::AccelerationStructureTypeKHR::TOP_LEVEL;
        
        let geometry_info = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
                    .data(vk::DeviceOrHostAddressConstKHR {
                        device_address: instance_buffer_address,
                    })
                    .build(),
            })
            .build();

        let build_range_infos = vec![vk::AccelerationStructureBuildRangeInfoKHR::builder()
            .primitive_count(new_instance_count as _)
            .build()];

        // 4. fill geometry build info
        let geometry_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(BUILD_AS_TYPE)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(std::slice::from_ref(&geometry_info))
            // TODO: use Update mode
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .build();

        let max_primitive_counts = [new_instance_count as u32]; // have only one tlas
    
        self.rebuild_acceleration_structure(
            cb_raw,
            geometry_build_info,
            &build_range_infos,
            &max_primitive_counts,
            update_tlas,
            scratch_buffer,
        )
    }

    /// # Note
    /// 
    /// Scratch buffer is not optional here.
    /// During the update of some blas or tlas, the scratch buffer must be provided.
    /// Due to the fact that we only update tlas for now.
    /// 
    /// Update blas will be inefficient, prefer building a new blas.
    fn rebuild_acceleration_structure(
        &self,
        cb_raw: vk::CommandBuffer,
        mut geometry_build_info: vk::AccelerationStructureBuildGeometryInfoKHR,
        build_range_infos: &[vk::AccelerationStructureBuildRangeInfoKHR],
        max_primitive_counts: &[u32],
        update_as: &RayTracingAccelerationStructure,
        scratch_buffer: &RayTracingAccelerationScratchBuffer,
    ) {
        // get the update scratch buffer memory requirements
        let memory_requirements = unsafe {
            self.ray_tracing_extensions.acceleration_structure_khr
                .get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_build_info,
                    max_primitive_counts,
                )
        };

        assert!(
            memory_requirements.acceleration_structure_size as usize <= update_as.backing_buffer.desc.size,
            "Inadequate acceleration structure backing buffer size!"
        );

        let scratch_buffer = scratch_buffer.buffer.lock();

        assert!(
            memory_requirements.build_scratch_size as usize <= scratch_buffer.desc.size,
            "Inadequate acceleration structure scratch buffer size!"
        );

        geometry_build_info.dst_acceleration_structure = update_as.raw;
        geometry_build_info.scratch_data = vk::DeviceOrHostAddressKHR { device_address: scratch_buffer.device_address(self) };

        unsafe {
            self.ray_tracing_extensions.acceleration_structure_khr
                .cmd_build_acceleration_structures(
                    cb_raw,
                    std::slice::from_ref(&geometry_build_info),
                    std::slice::from_ref(&build_range_infos),
                );
            
            // wait for building complete
            self.raw.cmd_pipeline_barrier(
                cb_raw,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::builder()
                    .src_access_mask(
                        vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR |
                        vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR |
                        vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                    )
                    .build()],
                &[],
                &[],
            );
        }
    }

    /// Reture the address of the instance buffer.
    pub fn fill_ray_tracing_tlas_instance_buffer(
        &self,
        dynamic_constants: &mut DynamicBuffer,
        instances: &[RayTracingBlasInstance],
    ) -> vk::DeviceAddress {
        let inst_buffer_address = dynamic_constants.current_device_address(self);

        // same thing in build_tlas()
        // but we store instance buffer data in dynamic buffer this time
        dynamic_constants.push_from_iter(instances.iter().map(|inst| {
            // row major
            let transform_mat = [
                inst.affine_xform.x_axis.x,
                inst.affine_xform.y_axis.x,
                inst.affine_xform.z_axis.x,
                inst.affine_xform.translation.x,
                inst.affine_xform.x_axis.y,
                inst.affine_xform.y_axis.y,
                inst.affine_xform.z_axis.y,
                inst.affine_xform.translation.y,
                inst.affine_xform.x_axis.z,
                inst.affine_xform.y_axis.z,
                inst.affine_xform.z_axis.z,
                inst.affine_xform.translation.z,
            ];

            let blas_address = unsafe {
                self.ray_tracing_extensions.acceleration_structure_khr.get_acceleration_structure_device_address(
                    &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
                        .acceleration_structure(inst.blas.raw)
                        .build(),
                )
            };

            RayTracingGeometryInstance::new(
                transform_mat,
                inst.mesh_index, // each mesh is a instance
                0xff, // full mask by default
                0, // offset will be filled later
                vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
                blas_address
            )
        }));

        inst_buffer_address as vk::DeviceAddress
    }

    pub fn destroy_ray_tracing_scratch_buffer(
        &self,
        scratch_buffer: RayTracingAccelerationScratchBuffer,
    ) {
        let inner_buffer = Arc::try_unwrap(scratch_buffer.buffer)
            .expect("Failed to destroy scratch buffer, someone is still using it!");
        let inner_buffer = Mutex::into_inner(inner_buffer);

        self.destroy_buffer(inner_buffer);
    }

    pub fn destroy_acceleration_structure(
        &self,
        accel_struct: RayTracingAccelerationStructure,
    ) {
        unsafe {
            self.ray_tracing_extensions.acceleration_structure_khr
                .destroy_acceleration_structure(accel_struct.raw, None)
        }
        
        self.destroy_buffer(accel_struct.backing_buffer);

        if let Some(init_instance_buffer) = accel_struct.init_instance_buffer.into_inner() {
            self.destroy_buffer(init_instance_buffer);
        }
    }
}