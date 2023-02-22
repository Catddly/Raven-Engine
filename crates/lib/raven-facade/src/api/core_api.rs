use raven_core::ptr::SingletonRefPtr;

#[cfg(feature = "default_core_api")]
pub use super::default_core_api::*;

#[cfg(feature = "default_core_api")]
pub use CoreApiImpl as CoreApi;

pub fn get() -> &'static CoreApi {
    unsafe { CORE_API.get_ref() }
}

pub fn connect(core_api: &mut CoreApi) {
    unsafe {
        CORE_API.replace(core_api)
    }
}

static mut CORE_API: SingletonRefPtr<CoreApi> = SingletonRefPtr::new_empty();