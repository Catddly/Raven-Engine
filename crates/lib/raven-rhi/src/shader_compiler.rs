use std::path::{PathBuf};
use std::sync::Arc;

use turbosloth::*;
use shader_prepper::{IncludeProvider};
use relative_path::RelativePathBuf;
use bytes::Bytes;
use failure::Error as FailureError;
use anyhow::Context;

use raven_core::filesystem::{self, lazy};

use crate::backend::{ShaderBinary, ShaderSource, PipelineShaderStage, PipelineShaderDesc, ShaderBinaryStage};

struct ShaderIncludeProvider {
    ctx: RunContext,
}

impl IncludeProvider for ShaderIncludeProvider {
    type IncludeContext = String;

    fn get_include(
        &mut self,
        path: &str,
        parent_file: &Self::IncludeContext,
    ) -> std::result::Result<(String, Self::IncludeContext), FailureError> {
        // get the next include path
        let file_path = if let Some('/') = path.chars().next() {
            path.to_owned()
        } else {
            let mut folder: RelativePathBuf = parent_file.into();
            folder.pop();
            folder.join(path).as_str().to_string()
        };

        let bytes: Arc<Bytes> = smol::block_on(
            lazy::LoadFile::new(PathBuf::from(file_path.clone()))
                .into_lazy()
                .eval(&self.ctx)
        )?;

        Ok((String::from_utf8(bytes.to_vec())?, file_path))
    }
}

// Lazy functor to compile shader from ShaderSource.
#[derive(Clone, Hash)]
pub struct CompileShader {
    pub source: PathBuf,
    pub profile: String,
    pub entry: String,
    pub force_recompile: bool,
}

impl Default for CompileShader {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            profile: String::new(),
            entry: String::new(),
            force_recompile: true,
        }
    }
}

#[async_trait]
impl LazyWorker for CompileShader {
    type Output = anyhow::Result<ShaderBinary>;
    
    async fn run(self, ctx: RunContext) -> Self::Output {
        //glog::debug!("Run {:?} on thread: {:?}", self.source, std::thread::current().name());

        let ext = self
            .source
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .expect(format!("Failed to find extension of {:?}", self.source).as_str());

        let name = self
            .source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .expect(format!("Failed to find file stem of {:?}", self.source).as_str());

        let spv_name = PathBuf::from(name.clone() + ".spv");

        if !self.force_recompile {
            // check if the spv shader is exists
            if filesystem::exist(&spv_name, filesystem::ProjectFolder::ShaderBinary)? {
                // just load the compiled spv.
                // TODO: notice that this can be the old version of current shader, need to cache the file version.
                let spirv = lazy::LoadFile::new(self.source.clone()).run(ctx).await?;

                let name = PathBuf::from(spv_name);
                return Ok(ShaderBinary { path: Some(name), spirv });
            }
        }

        match ext.as_str() {
            "spv" => {
                let spirv = lazy::LoadFile::new(self.source.clone()).run(ctx.clone()).await?;

                // store a copy in the ProjectFolder::ShaderBinary
                lazy::StoreFile::new(spirv.clone(), filesystem::ProjectFolder::ShaderBinary, spv_name.clone()).run(ctx).await?;

                Ok(ShaderBinary { path: Some(spv_name), spirv })
            }
            "glsl" => unimplemented!(),
            "hlsl" => {
                let file_name = PathBuf::from(self.source.to_str().unwrap().to_owned());
                let mut path = filesystem::get_project_folder_path_absolute(filesystem::ProjectFolder::ShaderSource)?;
                path.extend(file_name.iter());

                let source = shader_prepper::process_file(
                    &path.to_string_lossy(),
                    &mut ShaderIncludeProvider { ctx },
                    String::new(),
                );
                let source = source
                    .map_err(|err| anyhow::anyhow!("{}", err))
                    .with_context(|| format!("shader path: {:?}", self.source))?;
                let target_profile = format!("{}_6_4", self.profile);

                let mut source_text = String::new();
                for s in source {
                    source_text += &s.source;
                }
                let spirv = compile_shader_hlsl(&name, &source_text, &target_profile)?;

                let name = PathBuf::from(name + ".spv");
                Ok(ShaderBinary { path: Some(name), spirv })
            }
            _ => anyhow::bail!("Unrecognized shader file extension: {}", ext),
        }
    }
}

#[derive(Clone, Hash)]
pub struct CompileShaderStage {
    shaders: Vec<(PipelineShaderStage, CompileShader)>,
}

#[async_trait]
impl LazyWorker for CompileShaderStage {
    type Output = anyhow::Result<Vec<ShaderBinaryStage>>;
    
    async fn run(self, ctx: RunContext) -> Self::Output {
        //glog::debug!("Run batched on thread: {:?}", std::thread::current().name());

        let stages: Vec<_> = self.shaders.iter()
            .map(|(stage, compile_info)| (stage.clone(), compile_info.entry.clone()))
            .collect();

        let compiled_shaders: Vec<Arc<ShaderBinary>> = futures::future::try_join_all(self.shaders.into_iter()
            .map(|(_, shader)| {
                shader.into_lazy().eval(&ctx)
            })
        ).await?;

        Ok(compiled_shaders.into_iter()
            .zip(stages.into_iter())
            .map(|(binary, compile_info)| ShaderBinaryStage {
                stage: compile_info.0,
                entry: compile_info.1,
                binary: Some(binary.clone()),
            })
            .collect::<Vec<_>>())
    }
}

impl CompileShaderStage {
    pub fn builder() -> CompileShaderStagesBuilder {
        Default::default()
    }

    fn new(builder: CompileShaderStagesBuilder) -> Self {
        Self {
            shaders: builder.shaders,
        }
    }
}

pub struct CompileShaderStagesBuilder {
    shaders: Vec<(PipelineShaderStage, CompileShader)>,
}

impl CompileShaderStagesBuilder {
    pub fn add_stage(mut self, stage: PipelineShaderStage, source: PathBuf, entry: String, force_recompile: bool) -> Self {
        self.shaders.push((stage,
            CompileShader {
            source,
            profile: match stage {
                PipelineShaderStage::Vertex => "vs",
                PipelineShaderStage::Pixel => "ps",
            }.to_owned(),
            entry,
            force_recompile,
        }));

        self
    }

    pub fn with_pipeline_shader_desc(mut self, shaders: &[PipelineShaderDesc]) -> Self {
        for shader in shaders {
            let source = match &shader.source {
                ShaderSource::Glsl { path: _ } => { unimplemented!() },
                ShaderSource::Hlsl { path } => { path.clone() },
            };

            self.shaders.push((shader.stage,
                CompileShader {
                source,
                profile: match shader.stage {
                    PipelineShaderStage::Vertex => "vs",
                    PipelineShaderStage::Pixel => "ps",
                }.to_owned(),
                entry: shader.entry.clone(),
                ..Default::default()
            }))
        }

        self
    }

    pub fn build(self) -> CompileShaderStage {
        CompileShaderStage::new(self)   
    }
}

impl Default for CompileShaderStagesBuilder {
    fn default() -> Self {
        Self {
            shaders: Vec::new(),
        }
    }
}

fn compile_shader_hlsl(
    name: &str,
    source: &String,
    target_profile: &str,
) -> anyhow::Result<Bytes> {
    let t = std::time::Instant::now();

    let spirv = hassle_rs::compile_hlsl(
        name,
        &source,
        "main",
        target_profile,
        &[
            "-spirv",
            "-enable-templates",
            "-fspv-target-env=vulkan1.2", // hlsl for vulkan
            "-WX",  // warnings as errors
            "-Ges", // strict mode
        ],
        // TODO: add shader macro defines control to macro
        &[],
    )
    .map_err(|err| anyhow::anyhow!("{}", err))?;

    glog::info!("DX Compiler compile {} with {:?}", name, t.elapsed());

    Ok(spirv.into())
}
