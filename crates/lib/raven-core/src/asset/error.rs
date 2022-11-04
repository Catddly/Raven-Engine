use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetPipelineError {
    #[error("Asset pipeline failed on loading RawAsset!")]
    LoadFailure,

    // TODO: more useful error message
    #[error("Asset pipeline failed on processing RawAsset to Asset!")]
    ProcessFailure,

    #[error("Asset pipeline failed on baking StorageAsset to PackedAsset!")]
    BakeFailure,
}