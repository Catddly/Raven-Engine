use std::collections::HashMap;
use std::path::PathBuf;
use std::{sync::Arc};
use std::marker::PhantomData;

use once_cell::sync::Lazy;
use parking_lot::RwLock;


use super::asset_manager::ASSETS_MMAP;
use super::{Asset, VacantAsset, BakedAsset, Mesh, Texture, Material, AssetType, VecArrayQueryParam, TaggedAssetType};

type RegisterBoxAssetType = Box<dyn Asset + Send + Sync>;

const INVALID_ASSET_ID: u64 = u64::MAX;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AssetHandle {
    /// Unstable id depend on the register order of the asset.
    /// Only used for a handle to the asset registry, but should never be used as a global (in the disk) uuid.
    /// If you want to ident a resource using a global identifier, use AssetRef.
    id: u64,
    version: u64,
}

impl AssetHandle {
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl std::hash::Hash for AssetHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.id)
    }
}

impl std::ops::Deref for AssetHandle {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

#[derive(Clone, Debug)]
pub struct AssetRef<T: TaggedAssetType> {
    pub(crate) handle: Arc<AssetHandle>,
    pub(crate) uuid: u64,
    pub(crate) _marker: PhantomData<fn(&T)>,
}

impl<T: TaggedAssetType> AssetRef<T> {
    pub fn handle(&self) -> &Arc<AssetHandle> {
        &self.handle
    }

    pub fn uuid(&self) -> u64 {
        self.uuid
    }

    pub fn disk_ref<W>(&self) -> DiskAssetRef<W> {
        DiskAssetRef {
            uuid: self.uuid,
            _marker: PhantomData,
        }
    }
}

pub struct DiskAssetRef<T> {
    /// Use a stable hash value combining the uri (resource relative path) and the sub-resource sequence index as a uuid of the resource.
    uuid: u64,
    _marker: PhantomData<fn(&T)>,
}

impl<T> DiskAssetRef<T> {
    pub fn uuid(&self) -> u64 {
        self.uuid
    }
} 

impl<T> Clone for DiskAssetRef<T> {
    fn clone(&self) -> Self {
        Self {
            uuid: self.uuid,
            _marker: PhantomData,
        }
    }
}
impl<T> Copy for DiskAssetRef<T> {}

impl<T> PartialEq for DiskAssetRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid.eq(&other.uuid)
    }
}
impl<T> Eq for DiskAssetRef<T> {}

pub struct RuntimeAssetRegistry {
    current_id: u64,
    id_free_list: Vec<u64>,

    assets: Vec<RegisterBoxAssetType>,

    mesh_relative_mats: HashMap<AssetHandle, Vec<AssetRef<Material::Storage>>>,
    mesh_relative_texs: HashMap<AssetHandle, Vec<AssetRef<Texture::Storage>>>,
}

impl RuntimeAssetRegistry {
    fn new() -> Self {
        Self {
            current_id: 0,
            id_free_list: Default::default(),

            assets: Default::default(),

            mesh_relative_mats: Default::default(),
            mesh_relative_texs: Default::default(),
        }
    }

    pub fn register_asset(&mut self, asset: RegisterBoxAssetType) -> AssetHandle {
        let id = self.alloc_asset_id();
        self.assets[id as usize] = asset;

        let handle = AssetHandle {
            id,
            version: 0,
        };

        self.update_asset_refs(&handle);

        handle
    }

    #[inline]
    pub fn register_empty_asset(&mut self) -> AssetHandle {
        let id = self.alloc_asset_id();

        AssetHandle {
            id,
            version: 0,
        }
    }

    pub fn update_asset(&mut self, handle: &mut AssetHandle, asset: RegisterBoxAssetType) {
        self.assets[handle.id as usize] = asset;
        handle.version += 1;

        self.update_asset_refs(&handle);
    }

    pub fn get_asset(&self, handle: &AssetHandle) -> Option<&RegisterBoxAssetType> {
        self.assets.get(handle.id as usize)
    }

    pub fn get_asset_relative_materials(&self, handle: &AssetHandle) -> Option<&Vec<AssetRef<Material::Storage>>> {
        self.mesh_relative_mats.get(handle)
    }

    pub fn get_asset_relative_textures(&self, handle: &AssetHandle) -> Option<&Vec<AssetRef<Texture::Storage>>> {
        self.mesh_relative_texs.get(handle)
    }

    #[inline]
    pub fn get_baked_mesh_asset(&self, baked_asset: &BakedAsset) -> Mesh::FieldReader {
        let asset_mmap = ASSETS_MMAP.lock();
        let bytes: &[u8] = asset_mmap.get(&baked_asset.uri).unwrap();
        Mesh::get_field_reader(bytes)
    }

    #[inline]
    pub fn get_baked_texture_asset(&self, baked_asset: &BakedAsset) -> Texture::FieldReader {
        let asset_mmap = ASSETS_MMAP.lock();
        let bytes: &[u8] = asset_mmap.get(&baked_asset.uri).unwrap();
        Texture::get_field_reader(bytes)
    }

    #[inline]
    pub fn get_baked_material_asset(&self, baked_asset: &BakedAsset) -> Material::FieldReader {
        let asset_mmap = ASSETS_MMAP.lock();
        let bytes: &[u8] = asset_mmap.get(&baked_asset.uri).unwrap();
        Material::get_field_reader(bytes)
    }

    #[inline]
    pub fn is_valid(&self, handle: AssetHandle) -> bool {
        handle.id != INVALID_ASSET_ID
    }

    fn update_asset_refs(&mut self, handle: &AssetHandle) {
        let asset = self.get_asset(&handle).unwrap();
        
        match asset.asset_type() {
            AssetType::Mesh => {
                let mesh_asset = asset.as_mesh().unwrap();

                let mut mat_refs = Vec::with_capacity(mesh_asset.materials.len());
                for mat in mesh_asset.materials.iter() {
                    mat_refs.push(mat.clone());
                }
                
                let mut tex_refs = Vec::with_capacity(mesh_asset.material_textures.len());
                for tex in mesh_asset.material_textures.iter() {
                    tex_refs.push(tex.clone());
                }
                
                self.mesh_relative_mats.entry(*handle).or_insert(mat_refs);
                self.mesh_relative_texs.entry(*handle).or_insert(tex_refs);
            }
            AssetType::Baked => {
                let baked_asset = asset.as_baked().unwrap();
                let baked_asset_type = baked_asset.origin_asset_type();

                if let AssetType::Mesh = baked_asset_type {
                    let mesh_field_reader = self.get_baked_mesh_asset(baked_asset);
 
                    let mat_len = mesh_field_reader.materials(VecArrayQueryParam::length()).length();
                    let mut mat_refs = Vec::with_capacity(mat_len);

                    for idx in 0..mat_len {
                        let material_ref = mesh_field_reader.materials(VecArrayQueryParam::Index(idx)).value();
                        // this is a relative uri
                        let mat_uri = PathBuf::from(format!("{:8.8x}.mat", material_ref.uuid()));
                        
                        let mat_handle = self.register_asset(Box::new(BakedAsset { uri: mat_uri }));
                        
                        mat_refs.push(AssetRef {
                            handle: Arc::new(mat_handle),
                            uuid: material_ref.uuid(),
                            _marker: PhantomData,
                        });
                    }
                    self.mesh_relative_mats.entry(*handle).or_insert(mat_refs);

                    let tex_len = mesh_field_reader.material_textures(VecArrayQueryParam::length()).length();
                    let mut tex_refs = Vec::with_capacity(tex_len);

                    for idx in 0..tex_len {
                        let texture_ref = mesh_field_reader.material_textures(VecArrayQueryParam::Index(idx)).value();
                        // this is a relative uri
                        let tex_uri = PathBuf::from(format!("{:8.8x}.tex", texture_ref.uuid()));

                        let tex_handle = self.register_asset(Box::new(BakedAsset { uri: tex_uri }));

                        tex_refs.push(AssetRef {
                            handle: Arc::new(tex_handle),
                            uuid: texture_ref.uuid(),
                            _marker: PhantomData,
                        });
                    }
                    self.mesh_relative_texs.entry(*handle).or_insert(tex_refs);
                }
            }
            _ => {}
        }
    }

    fn alloc_asset_id(&mut self) -> u64 {
        if self.id_free_list.is_empty() {
            let id = self.current_id;
            self.current_id = self.current_id.checked_add(1).unwrap();
            // add a default empty asset
            self.assets.push(Box::new(VacantAsset {}));

            id
        } else {
            self.id_free_list.pop().unwrap()
        }
    }
}

/// Lazy static global singleton 
pub fn get_runtime_asset_registry() -> &'static RwLock<RuntimeAssetRegistry> {
    static RUNTIME_ASSET_MANAGER: Lazy<RwLock<RuntimeAssetRegistry>> = Lazy::new(|| {
        RwLock::new(RuntimeAssetRegistry::new())
    });

    &RUNTIME_ASSET_MANAGER
}