use std::sync::Arc;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use turbosloth::*;

use crate::{
    backend::{RasterPipeline, ShaderBinary, RasterPipelineDesc, ComputePipelineDesc, ComputePipeline, PipelineShaderDesc, ShaderSource, ShaderBinaryStage, RHIError, Device},
    shader_compiler::{CompileShader, CompileShaderStage}
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterPipelineHandle(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePipelineHandle(usize);

struct RasterPipelineEntry {
    desc: RasterPipelineDesc,
    pipeline: Option<Arc<RasterPipeline>>,
    // compile the shader when you need to build the pipeline.
    lazy_binary: Lazy<Vec<ShaderBinaryStage>>,
}

struct ComputePipelineEntry {
    desc: ComputePipelineDesc,
    pipeline: Option<Arc<ComputePipeline>>,
    // compile the shader when you need to build the pipeline.
    lazy_binary: Lazy<ShaderBinary>,
}

pub struct PipelineCache {
    lazy_cache: Arc<LazyCache>,

    raster_pipelines_entry: HashMap<RasterPipelineHandle, RasterPipelineEntry>,
    compute_pipelines_entry: HashMap<ComputePipelineHandle, ComputePipelineEntry>,

    // for fast backward search
    desc_to_raster_handle: HashMap<Vec<PipelineShaderDesc>, RasterPipelineHandle>,
    desc_to_compute_handle: HashMap<ComputePipelineDesc, ComputePipelineHandle>,

    raster_pipeline_spirv_cache: HashMap<RasterPipelineHandle, Arc<Vec<ShaderBinaryStage>>>,
    compute_pipeline_spirv_cache: HashMap<ComputePipelineHandle, Arc<ShaderBinary>>,
}

impl PipelineCache {
    pub fn new(lazy_cache: Arc<LazyCache>) -> Self {
        Self {
            lazy_cache,

            raster_pipelines_entry: HashMap::new(),
            compute_pipelines_entry: HashMap::new(),
            
            desc_to_raster_handle: HashMap::new(),
            desc_to_compute_handle: HashMap::new(),
        
            raster_pipeline_spirv_cache: HashMap::new(),
            compute_pipeline_spirv_cache: HashMap::new(),
        }
    }

    // is the order of parameter 'shaders' matters? (i.e. will its order affect desc_to_raster_handle?)
    pub fn register_raster_pipeline(&mut self, shaders: &[PipelineShaderDesc], desc: &RasterPipelineDesc) -> RasterPipelineHandle {
        // found a cached pipeline, just return it.
        if let Entry::Occupied(entry) = self.desc_to_raster_handle.entry(shaders.to_vec()) {
            return entry.get().clone()
        };

        let idx = self.raster_pipelines_entry.len();

        self.raster_pipelines_entry.insert(RasterPipelineHandle(idx), RasterPipelineEntry {
            desc: desc.clone(),
            pipeline: None,
            lazy_binary: CompileShaderStage::builder()
                .with_pipeline_shader_desc(shaders)
                .build()
                .into_lazy(),
        });
        self.desc_to_raster_handle.insert(shaders.to_vec(), RasterPipelineHandle(idx));

        RasterPipelineHandle(idx)
    }

    pub fn register_compute_pipeline(&mut self, desc: &ComputePipelineDesc) -> ComputePipelineHandle {
        // found a cached pipeline, just return it.
        if let Entry::Occupied(entry) = self.desc_to_compute_handle.entry(desc.clone()) {
            return entry.get().clone()
        };

        let idx = self.compute_pipelines_entry.len();

        let source = match &desc.source {
            ShaderSource::Glsl { path: _ } => { unimplemented!() },
            ShaderSource::Hlsl { path } => { path.clone() },
        };

        self.compute_pipelines_entry.insert(ComputePipelineHandle(idx), ComputePipelineEntry {
            desc: desc.clone(),
            pipeline: None,
            lazy_binary: CompileShader { 
                source,
                profile: "cs".to_owned(),
                ..Default::default()
            }.into_lazy(),
        });
        self.desc_to_compute_handle.insert(desc.clone(), ComputePipelineHandle(idx));

        ComputePipelineHandle(idx)
    }

    pub fn prepare(&mut self) -> anyhow::Result<()>{
        self.discard_stale_pipelines();
        self.parallel_compile_shaders()?;

        Ok(())
    }

    fn discard_stale_pipelines(&mut self) {
        // wipe out all the stale pipelines
        for (_, entry) in &mut self.raster_pipelines_entry {
            // if the shader binary is not up-to-date, the pipeline need to be reconstructed
            if !entry.lazy_binary.is_up_to_date() {
                if let Some(_pipe) = &mut entry.pipeline {
                    // TODO: release old pipeline
                    entry.pipeline = None;
                }
            }
        }
        
        for (_, entry) in &mut self.compute_pipelines_entry {
            // if the shader binary is not up-to-date, the pipeline need to be reconstructed
            if !entry.lazy_binary.is_up_to_date() {
                if let Some(_pipe) = &mut entry.pipeline {
                    // TODO: release old pipeline
                    entry.pipeline = None;
                }
            }
        }
    }

    fn parallel_compile_shaders(&mut self) -> anyhow::Result<()> {
        let raster_lazy_works = self.raster_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    smol::spawn(async move {
                        future.await
                            .map(|binaries| CompiledShaderOutput::Raster { handle, binaries })
                    })
                })
            });

        let compute_lazy_works = self.compute_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    smol::spawn(async move {
                        future.await
                            .map(|binary| CompiledShaderOutput::Compute { handle, binary })
                    })
                })
            });

        // notice that this is just iterator, we are not consuming it yet.
        let compiled_shaders_tasks = raster_lazy_works.chain(compute_lazy_works).collect::<Vec<_>>();

        if !compiled_shaders_tasks.is_empty() {
            // compile all shaders
            let compiled_shaders = smol::block_on(futures::future::try_join_all(compiled_shaders_tasks))
                .map_err(|err| anyhow::anyhow!("Failed to compiler shader with: {:?}", err))?;

            for compiled in compiled_shaders {
                // cache all the compiled spirv binaries
                match compiled {
                    CompiledShaderOutput::Raster { handle, binaries } => {
                        self.raster_pipeline_spirv_cache.insert(handle, binaries);
                    },
                    CompiledShaderOutput::Compute { handle, binary } => {
                        self.compute_pipeline_spirv_cache.insert(handle, binary);
                    }
                }
            }
        }

        Ok(())
    }

    fn update_pipelines(&mut self, device: Arc<Device>) {
        for (handle, cache) in &mut self.raster_pipeline_spirv_cache {
            
        }
    }
}

enum CompiledShaderOutput {
    Raster {
        handle: RasterPipelineHandle, 
        binaries: Arc<Vec<ShaderBinaryStage>>,
    },
    Compute {
        handle: ComputePipelineHandle,
        binary: Arc<ShaderBinary>,
    },
}