use std::{path::PathBuf, sync::Arc};

use thiserror::Error;

use crate::asset::RawAsset;

#[derive(Debug, Clone, Hash)]
pub enum LoadAssetTextureType {
    Unknown,
    Png,
    Dds,
    Jpg,
    Tex, // Baked Engine Texture Type
}

#[derive(Debug, Clone, Hash)]
pub enum LoadAssetMeshType {
    Gltf,
    Obj,
}

#[derive(Debug, Clone, Hash)]
pub enum LoadAssetSceneType {
    RavenScene,
    JsonScene,
}

#[derive(Debug, Clone, Hash)]
pub enum LoadAssetMaterialType {
    Mat, // Baked Engine Material Type
}

#[derive(Debug, Clone, Hash)]
pub enum LoadAssetType {
    Texture(LoadAssetTextureType),
    Mesh(LoadAssetMeshType),
    Scene(LoadAssetSceneType),
    Material(LoadAssetMaterialType),
}

#[derive(Debug, Error)]
pub enum AssetLoaderError {
    #[error("Failed to extract extension from {path:?}!")]
    InvalidExtension { path: PathBuf },

    #[error("Unsupported mesh type : {path:?}")]
    UnsupportedMeshType { path: PathBuf }
}

pub(crate) fn extract_asset_type(name: &PathBuf) -> LoadAssetType {
    let try_mesh = extract_mesh_type(name);
    if try_mesh.is_err() {
        let try_tex = extract_texture_type(name);
        if try_tex.is_err() {
            let try_mat = extract_material_type(name);
            if try_mat.is_err() {
                panic!("Unsupported asset type!");
            } else {
                LoadAssetType::Material(try_mat.unwrap())
            }
        } else {
            LoadAssetType::Texture(try_tex.unwrap())
        }
    } else {
        LoadAssetType::Mesh(try_mesh.unwrap())
    }
}

pub(crate) fn extract_material_type(name: &PathBuf) -> anyhow::Result<LoadAssetMaterialType, AssetLoaderError> {
    let ext = name.extension()
        .ok_or(AssetLoaderError::InvalidExtension { path: name.clone() } )?;
    let ext = ext.to_str().unwrap();

    match ext {
        "mat" => Ok(LoadAssetMaterialType::Mat),
        _ => Err(AssetLoaderError::UnsupportedMeshType { path: name.clone() })
    }
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

pub(crate) fn extract_texture_type(name: &PathBuf) -> anyhow::Result<LoadAssetTextureType, AssetLoaderError> {
    let ext = name.extension()
        .ok_or(AssetLoaderError::InvalidExtension { path: name.clone() } )?;
    let ext = ext.to_str().unwrap();

    match ext {
        "jpg" | "jpeg" => Ok(LoadAssetTextureType::Jpg),
        "png" => Ok(LoadAssetTextureType::Png),
        "dds" => Ok(LoadAssetTextureType::Dds),
        "tex" => Ok(LoadAssetTextureType::Tex),
        _ => Err(AssetLoaderError::UnsupportedMeshType { path: name.clone() })
    }
}

pub trait AssetLoader {
    fn load(&self) -> anyhow::Result<Arc<dyn RawAsset + Send + Sync>>;

    fn get_load_uri(&self) -> PathBuf;
}