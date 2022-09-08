use std::{ptr, os::raw::c_void, ffi::CStr};

use ash::{vk};

use super::utility::*;

/// Vulkan debug callback function.
/// use extern "system" here for stdcall abi capability.
unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let types = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
        _ => "[Unknown]",
    };
    let message = CStr::from_ptr((*p_callback_data).p_message);

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => glog::trace!("{}: {:?}", types, message),
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => glog::info!("[{}: {:?}", types, message),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => glog::warn!("{}: {:?}", types, message),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => { glog::error!("{}: {:?}", types, message); debug_assert!(false) },
        _ => unreachable!(),
    };
    vk::FALSE
}

/// Populate Vulkan debug messager info.
pub fn populate_debug_messenger_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT {
    vk::DebugUtilsMessengerCreateInfoEXT {
        s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
        p_next: ptr::null(),
        flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING |
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        pfn_user_callback: Some(vulkan_debug_utils_callback),
        p_user_data: ptr::null_mut(),
    }
}

/// If support validation layer, then return true
pub fn check_validation_layer_support(
    entry: &ash::Entry,
    required_validation_layers: &Vec<&str>,
) -> bool {
    let layer_properties = entry
        .enumerate_instance_layer_properties()
        .expect("Failed to enumerate Instance Layers Properties");

    if layer_properties.len() <= 0 {
        eprintln!("No available layers.");
        return false;
    }

    for required_layer_name in required_validation_layers.iter() {
        let mut is_layer_found = false;

        for layer_property in layer_properties.iter() {
            let test_layer_name = vk_to_string(&layer_property.layer_name);
            if (*required_layer_name) == test_layer_name {
                is_layer_found = true;
                break;
            }
        }

        if is_layer_found == false {
            return false;
        }
    }

    true
}

/// Enable basic debug utilities.
pub fn setup_debug_utils(
    is_enable_debug: bool,
    entry: &ash::Entry,
    instance: &ash::Instance,
) -> (ash::extensions::ext::DebugUtils, vk::DebugUtilsMessengerEXT) {
    let debug_utils_loader = ash::extensions::ext::DebugUtils::new(entry, instance);

    if is_enable_debug == false {
        (debug_utils_loader, ash::vk::DebugUtilsMessengerEXT::null())
    } else {
        let messenger_ci = populate_debug_messenger_create_info();

        let utils_messenger = unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&messenger_ci, None)
                .expect("Failed to create vulkan DebugUtilsMessengerEXT!")
        };

        (debug_utils_loader, utils_messenger)
    }
}