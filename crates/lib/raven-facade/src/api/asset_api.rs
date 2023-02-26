use raven_core::ptr::SingletonRefPtr;

#[cfg(feature = "default_asset_api")]
pub use super::default_asset_api::*;

#[cfg(feature = "default_asset_api")]
pub use AssetApiImpl as AssetApi;

pub fn get() -> &'static AssetApi {
    unsafe { ASSET_API.get_ref() }
}

pub fn connect(asset_api: &mut AssetApi) {
    unsafe {
        ASSET_API.replace(asset_api)
    }
}

static mut ASSET_API: SingletonRefPtr<AssetApi> = SingletonRefPtr::new_empty();