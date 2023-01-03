use std::{path::PathBuf, sync::Arc};
use std::collections::HashMap;

use parking_lot::Mutex;
use turbosloth::*;
use memmap2::{Mmap, MmapOptions};

use crate::{concurrent::executor, filesystem::{self, ProjectFolder}};

use super::error::AssetPipelineError;
use super::get_uri_bake_stem;
use super::loader::extract_asset_type;
use super::{
    loader::{
        LoadAssetType, extract_mesh_type, extract_texture_type, 
        AssetLoader, LoadAssetMeshType, mesh_loader::GltfMeshLoader, LoadAssetTextureType, texture_loader::JpgTextureLoader
    }, 
    RawAsset, asset_registry::{AssetHandle, get_runtime_asset_registry}, asset_process::AssetProcessor, asset_baker::AssetBaker, BakedAsset, BakedRawAsset
};

lazy_static::lazy_static! {
    pub(crate) static ref ASSETS_MMAP: Mutex<HashMap<PathBuf, Mmap>> = Mutex::new(HashMap::new());
}

#[derive(Hash, Clone, Debug)]
struct AssetPipelineKey(PathBuf);

impl From<PathBuf> for AssetPipelineKey {
    fn from(uri: PathBuf) -> Self {
        Self(uri)
    }
}

pub struct AssetManager {
    loaders: Mutex<Vec<Arc<dyn AssetLoader + Send + Sync>>>,
    //loader_groups: Mutex<Vec<LoadGroup>>,

    lazy_cache: Arc<LazyCache>,
}

pub struct AssetLoadDesc {
    pub load_ty: LoadAssetType,
    pub uri: PathBuf,
}

impl AssetLoadDesc {
    /// Assuming all asset have unique uri.
    /// (e.g. same name on .gltf and .obj)
    pub fn load_mesh(uri: impl Into<PathBuf>) -> Self {
        let uri = uri.into();
        if !uri.is_relative() {
            panic!("Invalid mesh uri! {:?}", uri);
        }

        let load_ty = extract_mesh_type(&uri).unwrap();
        Self {
            load_ty: LoadAssetType::Mesh(load_ty),
            uri,
        }
    }

    pub fn load_texture(uri: impl Into<PathBuf>) -> Self {
        let uri = uri.into();
        if !uri.is_relative() {
            panic!("Invalid texture uri! {:?}", uri);
        }

        let load_ty = extract_texture_type(&uri).unwrap();
        Self {
            load_ty: LoadAssetType::Texture(load_ty),
            uri,
        }
    }
} 

impl AssetManager {
    pub fn new() -> Self {
        filesystem::exist_or_create(ProjectFolder::Baked).expect("Failed to create Baked asset folder!");
        
        Self {
            loaders: Mutex::new(Vec::new()),
            //loader_groups: Mutex::new(Vec::new()),

            lazy_cache: LazyCache::create(),
        }
    }

    pub fn load_asset(&self, load_desc: AssetLoadDesc) -> anyhow::Result<()> {
        let is_baked = self.is_baked(&load_desc.uri);

        if let Some(baked) = is_baked {
            Self::mmap_baked_asset(&baked, &load_desc.uri)?;

            let mut registry = get_runtime_asset_registry().write();
            // add the baked asset immediately
            let handle = registry.register_asset(Box::new(BakedAsset { uri: load_desc.uri.clone() }));
            let bake_folder = filesystem::get_project_folder_path_absolute(ProjectFolder::Baked)?;
            
            if let Some(mat_refs) = registry.get_asset_relative_materials(&handle) {
                for mat_ref in mat_refs {
                    let uri = PathBuf::from(format!("{:8.8x}.mat", mat_ref.uuid()));
                    let baked = bake_folder.join(uri.clone());
    
                    Self::mmap_baked_asset(&baked, &uri)?;
                }
            }

            if let Some(tex_refs) = registry.get_asset_relative_textures(&handle) {
                for tex_ref in tex_refs {
                    let uri = PathBuf::from(format!("{:8.8x}.tex", tex_ref.uuid()));
                    let baked = bake_folder.join(uri.clone());
    
                    Self::mmap_baked_asset(&baked, &uri)?;
                }
            }

            let mut loaders = self.loaders.lock();
            let AssetLoadDesc { uri, load_ty: _ } = load_desc;

            // push a dummy task, it actually do nothing but just return the existed AssetHandle
            loaders.push(Arc::new(BakedAssetLoader { handle, uri }));

            return Ok(());
        }

        let mut loaders = self.loaders.lock();
        let AssetLoadDesc { uri, load_ty } = load_desc;

        match load_ty {
            LoadAssetType::Mesh(mesh_ty) => {
                match mesh_ty {
                    LoadAssetMeshType::Gltf => { 
                        loaders.push(Arc::new(GltfMeshLoader::new(uri)));
                    }
                    LoadAssetMeshType::Obj => { 
                        unimplemented!()
                    }
                }
            }
            LoadAssetType::Texture(tex_ty) => {
                match tex_ty {
                    LoadAssetTextureType::Jpg => {
                        // TODO: expose params
                        loaders.push(Arc::new(JpgTextureLoader::new(uri).generate_mipmap(true)));
                    }
                    _ => unimplemented!()
                }
            }
            _ => unimplemented!()
        }

        Ok(())
    }

    pub fn dispatch_load_tasks(&self) -> anyhow::Result<Vec<Arc<AssetHandle>>> {
        // TODO: optimize this
        let mut loaders = self.loaders.lock();
        let load_tasks = loaders.drain(..);
          
        let tasks_iter = load_tasks.into_iter()
            .map(|worker| { 
                let load_asset = LoadRawAsset { worker };
                executor::spawn(load_asset.into_lazy().eval(&self.lazy_cache))
            });

        let tasks = smol::block_on(futures::future::try_join_all(tasks_iter))?;

        let uris = tasks.iter()
            .map(|loaded_raw| loaded_raw.key.0.clone())
            .collect::<Vec<_>>();

        let tasks_iter = tasks.into_iter()
            .map(|loaded_raw| {
                let process_asset = AssetProcessor::new(loaded_raw.key.0.clone(), loaded_raw.raw_asset.clone());
                executor::spawn(process_asset.process().unwrap().eval(&self.lazy_cache))
            })
            .collect::<Vec<_>>();

        let tasks = smol::block_on(futures::future::try_join_all(tasks_iter))?;

        let tasks_iter = tasks.iter().cloned().zip(uris.into_iter())
            .map(|(asset, uri)| {
                let baker = AssetBaker::new(asset, uri);
                executor::spawn(baker.into_lazy().eval(&self.lazy_cache))
            });

        smol::block_on(futures::future::try_join_all(tasks_iter))?;

        Ok(tasks)
    }

    fn mmap_baked_asset(baked_path: &PathBuf, uri: &PathBuf) -> anyhow::Result<(), AssetPipelineError> {
        let file = std::fs::File::open(baked_path.clone()).expect("Failed to open the baked file path!");
        let mmap = unsafe {
            MmapOptions::new().map(&file).map_err(|err| AssetPipelineError::LoadFailure { err })
        }?;

        // use origin uri here
        ASSETS_MMAP.lock().entry(uri.clone()).or_insert_with(|| mmap);

        Ok(())
    }

    fn is_baked(&self, uri: &PathBuf) -> Option<PathBuf> {
        let load_asset_type = extract_asset_type(uri);
        let mut baked_asset_name = get_uri_bake_stem(uri);

        match load_asset_type {
            LoadAssetType::Mesh(_) => {
                baked_asset_name.set_extension("mesh");
            }
            LoadAssetType::Texture(_) => {
                baked_asset_name.set_extension("tex");
            },
            _ => unimplemented!()
        }

        if filesystem::exist(&baked_asset_name, ProjectFolder::Baked).unwrap() {
            let folder = filesystem::get_project_folder_path_absolute(ProjectFolder::Baked).unwrap();
            let file = folder.join(baked_asset_name);

            Some(file)
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct LoadRawAsset {
    worker: Arc<dyn AssetLoader + Send + Sync>,
}

impl std::hash::Hash for LoadRawAsset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.worker.get_load_uri().hash(state)
    }
}

#[derive(Clone)]
struct LoadedRawAsset {
    raw_asset: Arc<dyn RawAsset>,
    key: AssetPipelineKey,
}

impl std::hash::Hash for LoadedRawAsset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

#[async_trait]
impl LazyWorker for LoadRawAsset {
    type Output = anyhow::Result<LoadedRawAsset>;

    async fn run(self, _ctx: RunContext) -> Self::Output {
        let loaded_asset = LoadedRawAsset {
            key: self.worker.get_load_uri().into(),
            raw_asset: self.worker.load()?
        };

        Ok(loaded_asset)
    }
}

struct BakedAssetLoader {
    uri: PathBuf,
    handle: AssetHandle,
}

impl AssetLoader for BakedAssetLoader {
    fn load(&self) -> anyhow::Result<Arc<dyn RawAsset>> {
        Ok(Arc::new(BakedRawAsset { handle: self.handle }))
    }

    fn get_load_uri(&self) -> PathBuf {
        self.uri.clone()
    }
}