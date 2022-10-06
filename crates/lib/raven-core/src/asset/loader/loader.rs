use std::{path::PathBuf};

use thiserror::Error;

use crate::asset::RawAsset;

#[derive(Debug)]
pub enum LoadAssetImageType {
    Png,
    Dds,
}

#[derive(Debug)]
pub enum LoadAssetMeshType {
    Gltf,
    Obj,
}

#[derive(Debug)]
pub enum LoadAssetSceneType {
    RavenScene,
    JsonScene,
}

#[derive(Debug)]
pub enum LoadAssetType {
    Image(LoadAssetImageType),
    Mesh(LoadAssetMeshType),
    Scene(LoadAssetSceneType)
}

#[derive(Debug, Error)]
pub enum AssetLoaderError {
    #[error("Failed to extract extension from {path:?}!")]
    InvalidExtension { path: PathBuf },

    #[error("Unsupported mesh type : {path:?}")]
    UnsupportedMeshType { path: PathBuf }
}

pub(crate) fn extract_mesh_type(name: &PathBuf) -> anyhow::Result<LoadAssetMeshType, AssetLoaderError> {
    let ext = name.extension()
        .ok_or(AssetLoaderError::InvalidExtension { path: name.clone() } )?;
    let ext = ext.to_str().unwrap();

    match ext {
        "gltf" | "glb" => Ok(LoadAssetMeshType::Gltf),
        "obj" => Ok(LoadAssetMeshType::Obj),
        _ => Err(AssetLoaderError::UnsupportedMeshType { path: name.clone() })
    }
}

pub trait AssetLoader {
    fn load(&self) -> anyhow::Result<Box<dyn RawAsset>>;
}