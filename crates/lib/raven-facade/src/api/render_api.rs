use raven_core::ptr::SingletonRefPtr;

#[cfg(feature = "default_render_api")]
pub use super::default_render_api::*;

#[cfg(feature = "default_render_api")]
pub use RenderApiImpl as RenderApi;

pub fn get() -> &'static RenderApi {
    unsafe { RENDER_API.get_ref() }
}

pub fn connect(render_api: &mut RenderApi) {
    unsafe {
        RENDER_API.replace(render_api)
    }
}

static mut RENDER_API: SingletonRefPtr<RenderApi> = SingletonRefPtr::new_empty();