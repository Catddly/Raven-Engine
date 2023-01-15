use std::ffi::CStr;

#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;

use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::Surface;

// Only support windows for now.
#[cfg(all(windows))]
pub fn required_extension_names() -> Vec<&'static CStr> {
    vec![
        Surface::name(),
        Win32Surface::name(),
        DebugUtils::name(),
    ]
}

#[cfg(all(windows))]
pub unsafe fn create_surface(
    entry: &ash::Entry,
    instance: &ash::Instance,
    window: &winit::window::Window,
) -> anyhow::Result<ash::vk::SurfaceKHR> {
    use std::os::raw::c_void;
    use std::ptr;
    use winapi::shared::windef::HWND;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winit::platform::windows::WindowExtWindows;

    let hwnd = window.hwnd() as HWND as *const c_void;
    let hinstance = GetModuleHandleW(ptr::null()) as *const c_void;
    let win32_create_info = ash::vk::Win32SurfaceCreateInfoKHR::builder()
        .hinstance(hinstance)
        .hwnd(hwnd)
        .build();

    let win32_surface_loader = Win32Surface::new(&entry, &instance);
    let result =Ok(win32_surface_loader.create_win32_surface(&win32_create_info, None)?);

    glog::trace!("Vulkan surface created!");
    result
}