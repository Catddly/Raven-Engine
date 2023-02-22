use raven_core::ptr::SingletonRefPtr;

#[cfg(feature = "default_input_api")]
pub use super::default_input_api::*;

#[cfg(feature = "default_input_api")]
pub use InputApiImpl as InputApi;

pub fn get() -> &'static InputApi {
    unsafe { INPUT_API.get_ref() }
}

pub fn connect(input_api: &mut InputApi) {
    unsafe {
        INPUT_API.replace(input_api)
    }
}

static mut INPUT_API: SingletonRefPtr<InputApi> = SingletonRefPtr::new_empty();