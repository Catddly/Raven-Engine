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

pub struct ShaderBinary {
    // This path must in ProjectFolder::ShaderBinary
    pub path: Option<PathBuf>,
    pub spirv: Bytes,
}

pub struct ShaderBinaryStage {
    pub stage: PipelineShaderStage,
    pub binary: Arc<ShaderBinary>,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub enum PipelineShaderStage {
    Vertex,
    Pixel,
}

#[derive(Builder, Clone, Hash, Eq, PartialEq, Debug)]
#[builder(pattern = "owned", derive(Clone))]
pub struct PipelineShaderDesc {
    pub stage: PipelineShaderStage,
    #[builder(default)]
    pub push_constants_bytes: usize, // push constants for the according shader stage.
    #[builder(default = "\"main\".to_owned()")]
    pub entry: String,
    pub source: ShaderSource,
}