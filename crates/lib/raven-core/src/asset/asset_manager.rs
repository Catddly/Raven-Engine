use std::{path::PathBuf, ops::{Range}, sync::Arc};
use std::collections::HashMap;

use parking_lot::Mutex;
use turbosloth::*;
use memmap2::{Mmap, MmapOptions};
use unsafe_any::UnsafeAny;

use crate::{concurrent::executor, filesystem::{self, ProjectFolder, lazy::LoadFile}};


use super::{
    loader::{
        LoadAssetType, extract_mesh_type, extract_texture_type, 
        AssetLoader, LoadAssetMeshType, mesh_loader::GltfMeshLoader, LoadAssetTextureType, texture_loader::JpgTextureLoader, extract_asset_type
    }, 
    error::AssetPipelineError, RawAsset, asset_registry::{AssetHandle, get_runtime_asset_registry}, asset_process::AssetProcessor, asset_baker::AssetBaker, AssetType, Texture, Mesh, BakedAsset, BakedRawAsset
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

enum LoadGroup {
    Single(usize),
    Batch(Range<usize>),
}

pub struct LoadGroupHandle(usize);

pub struct AssetManager {
    loaders: Mutex<Vec<Arc<dyn AssetLoader + Send + Sync>>>,
    loader_groups: Mutex<Vec<LoadGroup>>,

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
        Self {
            loaders: Mutex::new(Vec::new()),
            loader_groups: Mutex::new(Vec::new()),

            lazy_cache: LazyCache::create(),
        }
    }

    pub fn load_asset(&self, load_desc: AssetLoadDesc) {
        let is_baked = self.is_baked(&load_desc.uri);
        if let Some(baked) = is_baked {
            let file = std::fs::File::open(baked.clone()).expect("Failed to open the baked file path!");
            let mmap = unsafe {
                MmapOptions::new().map(&file).unwrap_or_else(|err| panic!("Failed to mmap {:?} with {:?}", baked, err))
            };

            // use origin uri here
            ASSETS_MMAP.lock().entry(load_desc.uri.clone()).or_insert_with(|| mmap);

            let mut registry = get_runtime_asset_registry().write();
            // use origin uri here
            let handle = registry.register_asset(Box::new(BakedAsset { uri: load_desc.uri.clone() }));

            let mut loaders = self.loaders.lock();
            let AssetLoadDesc { uri, load_ty: _ } = load_desc;

            let offset = loaders.len();

            // use origin uri here
            loaders.push(Arc::new(BakedAssetLoader { handle, uri }));

            let mut loader_groups = self.loader_groups.lock();
            let _handle = LoadGroupHandle(loader_groups.len());
            loader_groups.push(LoadGroup::Single(offset));

            return;
        }

        let mut loaders = self.loaders.lock();
        let AssetLoadDesc { uri, load_ty } = load_desc;

        let offset = loaders.len();
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

        let mut loader_groups = self.loader_groups.lock();
        let _handle = LoadGroupHandle(loader_groups.len());
        loader_groups.push(LoadGroup::Single(offset));
        //handle
    }

    pub fn dispatch_load_tasks(&self) -> anyhow::Result<Vec<Arc<AssetHandle>>> {
        // TODO: optimize this
        let mut loaders = self.loaders.lock();
        let load_tasks = loaders.drain(..);
          
        //let mut load_groups = self.loader_groups.lock();
        //let load_groups = load_groups.drain(..);

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

    fn is_baked(&self, uri: &PathBuf) -> Option<PathBuf> {
        let asset_ty = extract_asset_type(uri);
        let baked_asset_name = match asset_ty {
            LoadAssetType::Mesh(_) => {
                let mut filename = PathBuf::from(uri.clone().file_name().unwrap());
                filename.set_extension("mesh");
                filename
            }
            LoadAssetType::Texture(_) => {
                let mut filename = PathBuf::from(uri.clone().file_name().unwrap());
                filename.set_extension("tex");
                filename
            }
            _ => unimplemented!()
        };

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
    raw_asset: Arc<dyn RawAsset + Send + Sync>,
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
    fn load(&self) -> anyhow::Result<Arc<dyn RawAsset + Send + Sync>> {
        Ok(Arc::new(BakedRawAsset { handle: self.handle }))
    }

    fn get_load_uri(&self) -> PathBuf {
        self.uri.clone()
    }
}