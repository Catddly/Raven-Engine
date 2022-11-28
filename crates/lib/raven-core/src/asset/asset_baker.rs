use std::sync::Arc;
use std::path::PathBuf;

use parking_lot::RwLockReadGuard;
use turbosloth::*;

use crate::filesystem;

use super::{asset_registry::{AssetHandle, self, RuntimeAssetRegistry}, error::AssetPipelineError, AssetType, Mesh, Material, Texture, get_uri_bake_stem};

#[derive(Clone, Hash)]
pub struct AssetBaker {
    origin_res_path: PathBuf,
    handle: Arc<AssetHandle>,
}

impl AssetBaker {
    pub fn new(handle: Arc<AssetHandle>, origin_res_path: impl Into<PathBuf>) -> Self {
        let origin_res_path = origin_res_path.into();
        assert!(origin_res_path.is_relative() && !origin_res_path.is_dir());

        Self {
            origin_res_path,
            handle,
        }
    }
}

#[async_trait]
impl LazyWorker for AssetBaker {
    type Output = anyhow::Result<()>;

    async fn run(self, _: RunContext) -> Self::Output {
        // try read storage from asset registry
        let registry = asset_registry::get_runtime_asset_registry();

        {
            let read_guard = registry.read();
            
            let storage = read_guard.get_asset(&self.handle).ok_or(AssetPipelineError::BakeFailure)?;
            let ty = storage.asset_type();

            // already baked! Just return and do nothing.
            if let AssetType::Baked = ty {
                return Ok(());
            }

            filesystem::exist_or_create(filesystem::ProjectFolder::Baked)?;

            let path = filesystem::get_project_folder_path_absolute(filesystem::ProjectFolder::Baked)?;
            let filename = get_uri_bake_stem(&self.origin_res_path);
            let mut path = path.join(filename);

            match ty {
                AssetType::Mesh => {
                    path.set_extension("mesh");

                    Self::bake_mesh_asset(&path, storage.as_mesh().unwrap(), &read_guard)?
                }
                AssetType::Texture => {
                    path.set_extension("tex");
                    
                    Self::bake_texture_asset(&path, storage.as_texture().unwrap())?
                }
                _ => {
                    unimplemented!()
                }
            }
        }

        Ok(())
    }
}

impl AssetBaker {
    fn bake_mesh_asset<'a>(path: &PathBuf, asset: &Mesh::Storage, read_guard: &RwLockReadGuard<'a, RuntimeAssetRegistry>) -> anyhow::Result<()> {
        // TODO: use StoreFile
        let mut file = std::fs::File::create(path)?;
        asset.write_packed(&mut file);

        // write relative asset if exists
        for mat_ref in asset.materials.iter() {
            let mat_storage = read_guard.get_asset(&mat_ref.handle).ok_or(AssetPipelineError::BakeFailure)?;
            let mat = mat_storage.as_material().unwrap();

            let mut path = path.clone();
            path.set_file_name(format!("{:8.8x}", mat_ref.uuid));
            path.set_extension("mat");

            Self::bake_material_asset(&path, mat)?;
        }

        for tex_ref in asset.material_textures.iter() {
            let tex_storage = read_guard.get_asset(&tex_ref.handle).ok_or(AssetPipelineError::BakeFailure)?;
            let tex = tex_storage.as_texture().unwrap();

            let mut path = path.clone();
            path.set_file_name(format!("{:8.8x}", tex_ref.uuid));
            path.set_extension("tex");

            Self::bake_texture_asset(&path, tex)?;
        }

        Ok(())
    }

    fn bake_material_asset(path: &PathBuf, asset: &Material::Storage) -> anyhow::Result<()> {
        let mut file = std::fs::File::create(path)?;
        asset.write_packed(&mut file);

        Ok(())
    }

    fn bake_texture_asset(path: &PathBuf, asset: &Texture::Storage) -> anyhow::Result<()> {
        let mut file = std::fs::File::create(path)?;
        asset.write_packed(&mut file);

        Ok(())
    }
}