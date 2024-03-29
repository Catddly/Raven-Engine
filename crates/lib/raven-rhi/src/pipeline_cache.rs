use std::sync::Arc;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use turbosloth::*;

use raven_thread::executor;

use crate::{
    backend::{
        self,
        RasterPipelineDesc, RasterPipeline,
        ComputePipelineDesc, ComputePipeline,
        ShaderBinary, PipelineShaderDesc, ShaderSource, ShaderBinaryStage,
        Device,
        pipeline::{self, CommonPipelinePtrs}},
    shader_compiler::{CompileShader, CompileShaderStage}
};
#[cfg(feature = "gpu_ray_tracing")]
use crate::backend::pipeline::{RayTracingPipelineDesc, RayTracingPipeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterPipelineHandle(usize);

impl std::fmt::Display for RasterPipelineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Raster Pipeline handle: {}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePipelineHandle(usize);

impl std::fmt::Display for ComputePipelineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Compute Pipeline handle: {}", self.0)
    }    
}

#[cfg(feature = "gpu_ray_tracing")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RayTracingPipelineHandle(usize);

#[cfg(feature = "gpu_ray_tracing")]
impl std::fmt::Display for RayTracingPipelineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ray Tracing Pipeline handle: {}", self.0)
    }    
}

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

#[cfg(feature = "gpu_ray_tracing")]
struct RayTracingPipelineEntry {
    desc: RayTracingPipelineDesc,
    pipeline: Option<Arc<RayTracingPipeline>>,
    // compile the shader when you need to build the pipeline.
    lazy_binary: Lazy<Vec<ShaderBinaryStage>>,
}

pub struct PipelineCache {
    /// Storing all the spirv binaries
    lazy_cache: Arc<LazyCache>,

    raster_pipelines_entry: HashMap<RasterPipelineHandle, RasterPipelineEntry>,
    compute_pipelines_entry: HashMap<ComputePipelineHandle, ComputePipelineEntry>,
    #[cfg(feature = "gpu_ray_tracing")]
    ray_tracing_pipelines_entry: HashMap<RayTracingPipelineHandle, RayTracingPipelineEntry>,

    /// for fast backward search
    desc_to_raster_handle: HashMap<Vec<PipelineShaderDesc>, RasterPipelineHandle>,
    desc_to_compute_handle: HashMap<ComputePipelineDesc, ComputePipelineHandle>,
    #[cfg(feature = "gpu_ray_tracing")]
    desc_to_ray_tracing_handle: HashMap<Vec<PipelineShaderDesc>, RayTracingPipelineHandle>,

    raster_pipeline_spirv_cache: HashMap<RasterPipelineHandle, Arc<Vec<ShaderBinaryStage>>>,
    compute_pipeline_spirv_cache: HashMap<ComputePipelineHandle, Arc<ShaderBinary>>,
    #[cfg(feature = "gpu_ray_tracing")]
    ray_tracing_pipeline_spirv_cache: HashMap<RayTracingPipelineHandle, Arc<Vec<ShaderBinaryStage>>>,

    defer_release_pipelines: [Vec<CommonPipelinePtrs>; backend::DEVICE_DRAW_FRAMES],
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            lazy_cache: LazyCache::create(),

            raster_pipelines_entry: HashMap::new(),
            compute_pipelines_entry: HashMap::new(),
            #[cfg(feature = "gpu_ray_tracing")]
            ray_tracing_pipelines_entry: HashMap::new(),
            
            desc_to_raster_handle: HashMap::new(),
            desc_to_compute_handle: HashMap::new(),
            #[cfg(feature = "gpu_ray_tracing")]
            desc_to_ray_tracing_handle: HashMap::new(),
        
            raster_pipeline_spirv_cache: HashMap::new(),
            compute_pipeline_spirv_cache: HashMap::new(),
            #[cfg(feature = "gpu_ray_tracing")]
            ray_tracing_pipeline_spirv_cache: HashMap::new(),

            defer_release_pipelines: Default::default(),
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

    pub fn get_raster_pipeline(&self, handle: RasterPipelineHandle) -> Arc<RasterPipeline> {
        self.raster_pipelines_entry
            .get(&handle)
            .unwrap()
            .pipeline
            .clone()
            .unwrap()
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

    pub fn get_compute_pipeline(&self, handle: ComputePipelineHandle) -> Arc<ComputePipeline> {
        self.compute_pipelines_entry
            .get(&handle)
            .unwrap()
            .pipeline
            .clone()
            .unwrap()
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub fn register_ray_tracing_pipeline(&mut self, shaders: &[PipelineShaderDesc], desc: &RayTracingPipelineDesc) -> RayTracingPipelineHandle {
        // found a cached pipeline, just return it.
        if let Entry::Occupied(entry) = self.desc_to_ray_tracing_handle.entry(shaders.to_vec()) {
            return entry.get().clone()
        };

        let idx = self.ray_tracing_pipelines_entry.len();

        self.ray_tracing_pipelines_entry.insert(RayTracingPipelineHandle(idx), RayTracingPipelineEntry {
            desc: desc.clone(),
            pipeline: None,
            lazy_binary: CompileShaderStage::builder()
                .with_pipeline_shader_desc(shaders)
                .build()
                .into_lazy(),
        });
        self.desc_to_ray_tracing_handle.insert(shaders.to_vec(), RayTracingPipelineHandle(idx));

        RayTracingPipelineHandle(idx)
    }

    #[cfg(feature = "gpu_ray_tracing")]
    pub fn get_ray_tracing_pipeline(&self, handle: RayTracingPipelineHandle) -> Arc<RayTracingPipeline> {
        self.ray_tracing_pipelines_entry
            .get(&handle)
            .unwrap()
            .pipeline
            .clone()
            .unwrap()
    }

    pub fn prepare(&mut self, device: &Device) -> anyhow::Result<()>{
        self.discard_stale_pipelines(&device);
        self.parallel_compile_shaders()?;

        Ok(())
    }

    pub fn update_pipelines(&mut self, device: &Device) {
        for (handle, cache) in self.raster_pipeline_spirv_cache.drain() {
            let raster_pipe_entry = self.raster_pipelines_entry.get_mut(&handle).unwrap();

            let raster_pipe = pipeline::create_raster_pipeline(&device, raster_pipe_entry.desc.clone(), cache.as_slice())
                .expect(format!("Failed to create new raster pipeline for {}", handle).as_str());

            raster_pipe_entry.pipeline = Some(Arc::new(raster_pipe));
        }
        
        for (handle, cache) in self.compute_pipeline_spirv_cache.drain() {
            let compute_pipe_entry = self.compute_pipelines_entry.get_mut(&handle).unwrap();
            
            let compute_pipe = pipeline::create_compute_pipeline(&device, compute_pipe_entry.desc.clone(), &cache)
                .expect(format!("Failed to create new compute pipeline for {}", handle).as_str());
            
            compute_pipe_entry.pipeline = Some(Arc::new(compute_pipe));
        }

        #[cfg(feature = "gpu_ray_tracing")]
        for (handle, cache) in self.ray_tracing_pipeline_spirv_cache.drain() {
            let ray_tracing_pipe_entry = self.ray_tracing_pipelines_entry.get_mut(&handle).unwrap();
            
            let ray_tracing_pipe = pipeline::create_ray_tracing_pipeline(&device, ray_tracing_pipe_entry.desc.clone(), cache.as_slice())
                .expect(format!("Failed to create new ray tracing pipeline for {}", handle).as_str());
            
            ray_tracing_pipe_entry.pipeline = Some(Arc::new(ray_tracing_pipe));
        }
    }

    fn discard_stale_pipelines(&mut self, device: &Device) {
        let device_frame_idx = device.get_device_frame_index() as usize;

        // destroy last frame's stale pipelines
        for stale_pipelines in self.defer_release_pipelines[device_frame_idx].drain(..) {
            pipeline::destroy_common_pipeline_ptrs(device, stale_pipelines);
        }

        // insert this frame's stale pipelines
        for (_, entry) in &mut self.raster_pipelines_entry {
            // if the shader binary is not up-to-date, the pipeline need to be reconstructed
            if !entry.lazy_binary.is_up_to_date() {
                if let Some(pipe) = entry.pipeline.take() {
                    // make sure no one is still using this pipeline
                    let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale raster pipeline!");
                    self.defer_release_pipelines[device_frame_idx].push(pipe.pipeline.pipeline_ptrs);

                    entry.pipeline = None;
                }
            }
        }
        
        for (_, entry) in &mut self.compute_pipelines_entry {
            // if the shader binary is not up-to-date, the pipeline need to be reconstructed
            if !entry.lazy_binary.is_up_to_date() {
                if let Some(pipe) = entry.pipeline.take() {
                    // make sure no one is still using this pipeline
                    let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale compute pipeline!");
                    self.defer_release_pipelines[device_frame_idx].push(pipe.pipeline.pipeline_ptrs);

                    entry.pipeline = None;
                }
            }
        }

        #[cfg(feature = "gpu_ray_tracing")]
        for (_, entry) in &mut self.ray_tracing_pipelines_entry {
            // if the shader binary is not up-to-date, the pipeline need to be reconstructed
            if !entry.lazy_binary.is_up_to_date() {
                if let Some(pipe) = entry.pipeline.take() {
                    // make sure no one is still using this pipeline
                    let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale ray tracing pipeline!");
                    self.defer_release_pipelines[device_frame_idx].push(pipe.pipeline.pipeline_ptrs);

                    entry.pipeline = None;
                }
            }
        }
    }

    #[cfg(feature = "gpu_ray_tracing")]
    fn parallel_compile_shaders(&mut self) -> anyhow::Result<()> {
        let raster_lazy_works = self.raster_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    executor::spawn(async move {
                        future.await
                            .map(|binaries| CompiledShaderOutput::Raster { handle, binaries })
                    })
                })
            });

        let compute_lazy_works = self.compute_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    executor::spawn(async move {
                        future.await
                            .map(|binary| CompiledShaderOutput::Compute { handle, binary })
                    })
                })
            });

        let ray_tracing_lazy_works = self.ray_tracing_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    executor::spawn(async move {
                        future.await
                            .map(|binaries| CompiledShaderOutput::RayTracing { handle, binaries })
                    })
                })
            });

        // notice that this is just iterator, we are not consuming it yet.
        let compiled_shaders_tasks = raster_lazy_works
            .chain(compute_lazy_works)
            .chain(ray_tracing_lazy_works)
            .collect::<Vec<_>>();

        if !compiled_shaders_tasks.is_empty() {
            // compile all shaders
            let compiled_shaders = smol::block_on(futures::future::try_join_all(compiled_shaders_tasks))
                .map_err(|err| anyhow::anyhow!("Failed to compiler shader with: {:?}", err))?;

            for compiled in compiled_shaders {
                // cache all the compiled spirv binaries
                match compiled {
                    CompiledShaderOutput::Raster { handle, binaries } => {
                        self.raster_pipeline_spirv_cache.insert(handle, binaries);
                    }
                    CompiledShaderOutput::Compute { handle, binary } => {
                        self.compute_pipeline_spirv_cache.insert(handle, binary);
                    }
                    CompiledShaderOutput::RayTracing { handle, binaries } => {
                        self.ray_tracing_pipeline_spirv_cache.insert(handle, binaries);
                    },
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "gpu_ray_tracing"))]
    fn parallel_compile_shaders(&mut self) -> anyhow::Result<()> {
        let raster_lazy_works = self.raster_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    executor::spawn(async move {
                        future.await
                            .map(|binaries| CompiledShaderOutput::Raster { handle, binaries })
                    })
                })
            });

        let compute_lazy_works = self.compute_pipelines_entry.iter()
            .filter_map(|(&handle, entry)| {
                entry.pipeline.is_none().then(|| {
                    let future = entry.lazy_binary.eval(&self.lazy_cache);
                    executor::spawn(async move {
                        future.await
                            .map(|binary| CompiledShaderOutput::Compute { handle, binary })
                    })
                })
            });

        // notice that this is just iterator, we are not consuming it yet.
        let compiled_shaders_tasks = raster_lazy_works
            .chain(compute_lazy_works)
            .collect::<Vec<_>>();

        if !compiled_shaders_tasks.is_empty() {
            // compile all shaders
            let compiled_shaders = smol::block_on(futures::future::try_join_all(compiled_shaders_tasks))
                .map_err(|err| anyhow::anyhow!("Failed to compiler shader with: {:?}", err))?;

            for compiled in compiled_shaders {
                // cache all the compiled spirv binaries
                match compiled {
                    CompiledShaderOutput::Raster { handle, binaries } => {
                        self.raster_pipeline_spirv_cache.insert(handle, binaries);
                    }
                    CompiledShaderOutput::Compute { handle, binary } => {
                        self.compute_pipeline_spirv_cache.insert(handle, binary);
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean all the pipelines.
    pub fn clean(self, device: &Device) {
        for (_, entry) in self.raster_pipelines_entry {
            if let Some(pipe) = entry.pipeline {
                // make sure no one is still using this pipeline
                let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale raster pipeline!");
                pipeline::destroy_raster_pipeline(&device, pipe);
            }
        }
        
        for (_, entry) in self.compute_pipelines_entry {
            if let Some(pipe) = entry.pipeline {
                // make sure no one is still using this pipeline
                let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale compute pipeline!");
                pipeline::destroy_compute_pipeline(&device, pipe);
            }
        }

        #[cfg(feature = "gpu_ray_tracing")]
        for (_, entry) in self.ray_tracing_pipelines_entry {
            if let Some(pipe) = entry.pipeline {
                // make sure no one is still using this pipeline
                let pipe = Arc::try_unwrap(pipe).expect("User holding a smart pointer to some stale ray tracing pipeline!");
                pipeline::destroy_ray_tracing_pipeline(&device, pipe);
            }
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
    #[cfg(feature = "gpu_ray_tracing")]
    RayTracing {
        handle: RayTracingPipelineHandle, 
        binaries: Arc<Vec<ShaderBinaryStage>>,
    }
}