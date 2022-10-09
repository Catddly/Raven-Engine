//use std::collections::HashMap;

use once_cell::sync::Lazy;
use parking_lot::RwLock;

use super::{Asset, VacantAsset};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AssetHandle {
    id: u64,
    version: u64,
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

type RegisterBoxAssetType = Box<dyn Asset + Send + Sync>;

const INVALID_ASSET_ID: u64 = u64::MAX;

pub struct RuntimeAssetRegistry {
    current_id: u64,
    id_free_list: Vec<u64>,

    assets: Vec<RegisterBoxAssetType>,
}

impl RuntimeAssetRegistry {
    fn new() -> Self {
        Self {
            current_id: 0,
            id_free_list: Default::default(),

            assets: Default::default(),
        }
    }

    pub fn register_asset(&mut self, asset: RegisterBoxAssetType) -> AssetHandle {
        let id = self.alloc_asset_id();
    
        self.assets[id as usize] = asset;

        AssetHandle {
            id,
            version: 0,
        }
    }

    #[inline]
    pub fn register_empty_asset(&mut self) -> AssetHandle {
        let id = self.alloc_asset_id();

        AssetHandle {
            id,
            version: 0,
        }
    }

    #[inline]
    pub fn update_asset(&mut self, handle: &mut AssetHandle, asset: RegisterBoxAssetType) {
        self.assets[handle.id as usize] = asset;

        handle.version += 1;
    }

    #[inline]
    pub fn get_asset(&self, handle: AssetHandle) -> Option<&RegisterBoxAssetType> {
        self.assets.get(handle.id as usize)
    }

    #[inline]
    pub fn is_valid(&self, handle: AssetHandle) -> bool {
        handle.id != INVALID_ASSET_ID
    }

    fn alloc_asset_id(&mut self) -> u64 {
        if self.id_free_list.is_empty() {
            let id = self.current_id;
            self.current_id.checked_add(1).unwrap();
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