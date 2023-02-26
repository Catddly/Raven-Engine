use std::sync::Arc;
use std::ops::Deref;

use parking_lot::RwLock;

use raven_asset::{AssetManager};

pub use raven_asset::{AssetLoadDesc, AssetType, asset_registry::AssetHandle, AsConcreteAsset, AsConcreteRawAsset};

pub struct AssetApiInner {
    asset_manager: AssetManager,
}

impl std::fmt::Debug for AssetApiInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug AssetApiInner")
    }
}

impl AssetApiInner {
    pub fn new() -> Self {
        Self {
            asset_manager: AssetManager::new(),
        }
    }

    #[inline]
    pub fn load_asset(&self, load_desc: AssetLoadDesc) -> anyhow::Result<()> {
        self.asset_manager.load_asset(load_desc)
    }

    #[inline]
    pub fn dispatch_load_tasks(&self) -> anyhow::Result<Vec<Arc<AssetHandle>>> {
        self.asset_manager.dispatch_load_tasks()
    }
}

#[derive(Clone)]
pub struct AssetApiImpl(Option<Arc<RwLock<AssetApiInner>>>);

unsafe impl Send for AssetApiImpl {}
unsafe impl Sync for AssetApiImpl {}

impl std::fmt::Debug for AssetApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Debug Default AssetApiImpl")
    }
}

impl Deref for AssetApiImpl {
    type Target = Arc<RwLock<AssetApiInner>>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl AssetApiImpl {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn init(&mut self) {
        self.0 = Some(Arc::new(RwLock::new(AssetApiInner::new())));
    }

    pub fn shutdown(mut self) {
        if let Some(inner) = self.0.take() {
            let inner = Arc::try_unwrap(inner)
                .expect("Reference counting of asset api may not be retained!");
            let inner = inner.into_inner();
            drop(inner);
        } else {
            panic!("Try to shutdown asset apis before initializing!");
        }
    }
}
