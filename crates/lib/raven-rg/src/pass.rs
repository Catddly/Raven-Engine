use std::marker::PhantomData;
use std::path::PathBuf;

use vk_sync::AccessType;
use ash::vk;

use raven_rhi::backend::{self, RHIError, ComputePipelineDesc, PipelineShaderDesc, RasterPipelineDesc};

use crate::graph_resource::{GraphComputePipelineHandle, RenderGraphComputePipeline, GraphRasterPipelineHandle, RenderGraphRasterPipeline};
use crate::resource::{ResourceView, ResourceDesc};

use super::graph_resource::{GraphResourceHandle, Handle, GraphResourceRef};
use super::resource_registry::ResourceRegistry;
use super::graph::RenderGraph;
use super::resource::{TypeEqualTo, Resource, SRV, UAV, RT};

pub type RenderFunc = dyn FnOnce(&mut RenderPassContext) -> anyhow::Result<(), RHIError>;

pub(crate) struct PassResourceAccessType {
    pub(crate) access_type: AccessType,
}

/// Resource handle of the resource in the render graph and the access type.
pub(crate) struct PassResourceHandle {
    pub handle: GraphResourceHandle,
    pub access: PassResourceAccessType,
}

/// Render pass context to give user to do custom command buffer recording ability and etc.
pub struct RenderPassContext<'a> {
    /// Command Buffer to record rendering commands to.
    pub cb: &'a vk::CommandBuffer,
    /// Context Relative Resources to be used inside this render pass. 
    pub resources: &'a mut ResourceRegistry,
}

/// Render Pass in the render graph.
/// Each Pass instructs how GPU should do rendering at a given region of time.
/// Each Pass may import some render resources and may output some too.
/// So Pass must hold the barrier transition infos between Passes.
pub(crate) struct Pass {
    /// Slot id of the passes in the render graph.
    pub id: usize,
    /// Name of this pass.
    pub name: String,
    /// Imported resources of this pass.
    pub inputs: Vec<PassResourceHandle>,
    /// Exported resources of this pass.
    pub outputs: Vec<PassResourceHandle>,
    /// Render callback function.
    pub render_func: Option<Box<RenderFunc>>,
}

impl Pass {
    /// Create a new empty pass.
    pub(crate) fn new_empty(id: usize, name: String) -> Self {
        Self {
            id,
            name,
            inputs: Vec::new(),
            outputs: Vec::new(),
            render_func: None,
        }
    }
}

/// Helper struct to build a Pass.
pub struct PassBuilder<'rg> {
    pub(crate) rg: &'rg mut RenderGraph,
    pub(crate) pass: Option<Pass>,
}

impl<'rg> Drop for PassBuilder<'rg> {
    /// When dropping, add the built pass back into the render graph to finish adding.
    fn drop(&mut self) {
        glog::debug!("Dropping PassBuilder!");
        self.rg.finish_add_pass(self.pass.take().unwrap());
    }
}

impl<'rg> PassBuilder<'rg> {
    /// Create a temporary resource used in this pass.
    pub fn create<Desc: ResourceDesc>(
        &mut self,
        desc: Desc,
    ) -> Handle<<Desc as ResourceDesc>::Resource> 
    where
        Desc: TypeEqualTo<Other = <<Desc as ResourceDesc>::Resource as Resource>::Desc>,
    {
        self.rg.new_resource(desc)
    }

    /// Read-In Resource to be used in this pass.
    /// Returns a Reference to the render graph with SRV (because it is for read purpose).
    /// Constrain: access_type must be the read operation.
    pub fn read<ResType: Resource>(
        &mut self, 
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, SRV> {
        assert!(backend::barrier::is_read_only_access(&access_type), "Invalid read access type: {:?}", &access_type);

        self.read_impl(handle, access_type)
    }

    /// Write-Out Resource to be exported in this pass.
    /// Returns a Reference to the render graph with UAV (because it is for write purpose).
    /// Constrain: access_type must be the write operation.
    pub fn write<ResType: Resource>(
        &mut self, 
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, UAV> {
        assert!(backend::barrier::is_write_only_access(&access_type), "Invalid write access type: {:?}", &access_type);

        self.write_impl(handle, access_type)

    }

    /// Read-In Resource to be used in this pass.
    /// Returns a Reference to the render graph with RT (it is a render target).
    /// Constrain: access_type must be the read operation.
    pub fn raster_read<ResType: Resource>(
        &mut self,
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, RT> {
        assert!(backend::barrier::is_read_only_raster_access(&access_type), "Invalid raster read access type: {:?}", &access_type);

        self.read_impl(handle, access_type)
    }

    /// Write-Out Resource to be exported in this pass.
    /// Returns a Reference to the render graph with RT (it is a render target).
    /// Constrain: access_type must be the write operation.
    pub fn raster_write<ResType: Resource>(
        &mut self,
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, RT> {
        assert!(backend::barrier::is_write_only_raster_access(&access_type), "Invalid raster write access type: {:?}", &access_type);

        self.write_impl(handle, access_type)
    }

    /// Add render function to this pass.
    pub fn render(
        mut self,
        func: impl (FnOnce(&mut RenderPassContext) -> anyhow::Result<(), RHIError>) + 'static,    
    ) {
        let pass = self.pass.as_mut().unwrap();
        
        let old_render_func = pass.render_func.replace(Box::new(func));

        assert!(old_render_func.is_none());
    }

    fn write_impl<ResType: Resource, ViewType: ResourceView>(
        &mut self,
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, ViewType> {
        let pass = self.pass.as_mut().unwrap();

        pass.outputs.push(PassResourceHandle {
            handle: handle.handle, // write to the old generation
            access: PassResourceAccessType {
                access_type,
            },
        });

        GraphResourceRef {
            handle: handle.handle.expired(), // after written, it is a new generation
            desc: handle.desc.clone(),
            _marker: PhantomData,
        }
    }

    fn read_impl<ResType: Resource, ViewType: ResourceView>(
        &mut self,
        handle: &mut Handle<ResType>,
        access_type: AccessType,
    ) -> GraphResourceRef<ResType, ViewType> {
        let pass = self.pass.as_mut().unwrap();

        pass.inputs.push(PassResourceHandle {
            handle: handle.handle,
            access: PassResourceAccessType {
                access_type,
            },
        });

        GraphResourceRef {
            handle: handle.handle,
            desc: handle.desc.clone(),
            _marker: PhantomData,
        }
    }
}

// Pipeline relative
impl<'rg> PassBuilder<'rg> {
    pub fn register_compute_pipeline(&mut self, path: impl Into<PathBuf>) -> GraphComputePipelineHandle {
        let desc = ComputePipelineDesc::builder()
            .source(path.into())
            .build()
            .unwrap();

        self.register_compute_pipeline_with_desc(desc)
    }

    pub(crate) fn register_compute_pipeline_with_desc(&mut self, desc: ComputePipelineDesc) -> GraphComputePipelineHandle {
        let idx = self.rg.compute_pipelines.len();

        // copy predefined descriptor set layouts to pipeline set layouts
        // for (set_idx, layout) in &self.rg.predefined_descriptor_set_layouts {
        //     desc.descriptor_set_opts[*set_idx as usize] = Some((
        //         *set_idx,
        //         DescriptorSetLayoutOpts::builder()
        //             .replace(layout.bindings.clone())
        //             .build()
        //             .unwrap(),
        //     ));
        // }

        self.rg.compute_pipelines.push(RenderGraphComputePipeline { desc });

        GraphComputePipelineHandle { idx }
    }

    pub fn register_raster_pipeline(&mut self, shaders: &[PipelineShaderDesc], desc: RasterPipelineDesc) -> GraphRasterPipelineHandle {
        let idx = self.rg.raster_pipelines.len();

        // for (set_idx, layout) in &self.rg.predefined_descriptor_set_layouts {
        //     desc.descriptor_set_opts[*set_idx as usize] = Some((
        //         *set_idx,
        //         DescriptorSetLayoutOpts::builder()
        //             .replace(layout.bindings.clone())
        //             .build()
        //             .unwrap(),
        //     ));
        // }

        self.rg.raster_pipelines.push(RenderGraphRasterPipeline {
            desc,
            stages: shaders.to_vec(),
        });

        GraphRasterPipelineHandle { idx }
    }
}