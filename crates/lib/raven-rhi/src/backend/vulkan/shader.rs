use std::{path::PathBuf};
use std::sync::Arc;

use bytes::Bytes;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum ShaderSource {
    // This path must in ProjectFolder::ShaderSource
    Hlsl { path: PathBuf },
    Glsl { path: PathBuf }
}

// I don't thing this a good idea
impl From<PathBuf> for ShaderSource {
    fn from(path: PathBuf) -> Self {
        // default shader language is hlsl
        ShaderSource::Hlsl { path }
    }
}

impl From<&str> for ShaderSource {
    fn from(s: &str) -> Self {
        ShaderSource::Hlsl { path: PathBuf::from(s) }
    }
}

pub struct ShaderBinary {
    // This path must in ProjectFolder::ShaderBinary
    pub path: Option<PathBuf>,
    pub spirv: Bytes,
}

// impl Drop for ShaderBinary {
//     fn drop(&mut self) {
//         if let Some(path) = &mut self.path {
//             glog::debug!("Shader binary {:?} dropped!", path);
//         } else {
//             glog::debug!("Shader binary dropped!");
//         }
//     }
// }

pub struct ShaderBinaryStage {
    /// For debug purpose
    pub source: PathBuf,
    pub stage: PipelineShaderStage,
    pub entry: String,
    pub binary: Option<Arc<ShaderBinary>>,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub enum PipelineShaderStage {
    Vertex,
    Pixel,
    #[cfg(feature = "gpu_ray_tracing")]
    RayGen,
    #[cfg(feature = "gpu_ray_tracing")]
    RayMiss,
    #[cfg(feature = "gpu_ray_tracing")]
    RayClosestHit,
    #[cfg(feature = "gpu_ray_tracing")]
    RayAnyHit,
    #[cfg(feature = "gpu_ray_tracing")]
    RayCallable,
}

#[derive(Builder, Clone, Hash, Eq, PartialEq, Debug)]
#[builder(pattern = "owned", derive(Clone))]
pub struct PipelineShaderDesc {
    pub stage: PipelineShaderStage,
    #[builder(default)]
    pub push_constants_bytes: usize, // push constants for the according shader stage.
    #[builder(setter(into), default = "\"main\".to_owned()")]
    pub entry: String,
    #[builder(setter(custom))]
    pub source: ShaderSource,
}

impl PipelineShaderDescBuilder {
    pub fn source(mut self, source: impl Into<ShaderSource>) -> Self {
        self.source = Some(source.into());
        self
    }
}

impl PipelineShaderDesc {
    pub fn builder() -> PipelineShaderDescBuilder {
        Default::default()
    }
}