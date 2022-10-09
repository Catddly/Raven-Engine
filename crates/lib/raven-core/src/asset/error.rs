use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetPipelineError {
    // TODO: more useful error message
    #[error("Asset pipeline failed on processing RawAsset to Asset!")]
    ProcessFailure,
}