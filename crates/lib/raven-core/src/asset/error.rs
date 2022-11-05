use std::io::Error;

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AssetPipelineError {
    #[error("Asset pipeline failed on loading RawAsset with {err:?}!")]
    LoadFailure { err: Error },

    // TODO: more useful error message
    #[error("Asset pipeline failed on processing RawAsset to Asset!")]
    ProcessFailure,

    #[error("Asset pipeline failed on baking StorageAsset to PackedAsset!")]
    BakeFailure,
}