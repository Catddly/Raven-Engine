use std::sync::Arc;
use ash::vk;

use crate::backend::vulkan::{Instance, Surface};

#[derive(Copy, Clone)]
pub struct QueueFamily {
    pub index: u32,
    pub properties: vk::QueueFamilyProperties,
}

pub struct PhysicalDevice {
    pub raw: vk::PhysicalDevice,
    pub(crate) instance: Arc<Instance>,
    // keep some necessary infos
    pub(crate) queue_families: Vec<QueueFamily>,
    pub features: vk::PhysicalDeviceFeatures,
    pub properties: vk::PhysicalDeviceProperties,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties,
}

pub fn enumerate_physical_devices(instance: &Arc<Instance>) -> Vec<PhysicalDevice> {
    // NOT support multiple GPUs for now!
    let physical_devices = unsafe { instance.raw.enumerate_physical_devices() }
        .expect("Failed to enumerate physical devices!");

    let physical_devices: Vec<PhysicalDevice> = physical_devices.into_iter()
        .map(|pd| {
            let features = unsafe { instance.raw.get_physical_device_features(pd) };
            let properties = unsafe { instance.raw.get_physical_device_properties(pd) };
            let memory_properties = unsafe { instance.raw.get_physical_device_memory_properties(pd) };

            let queue_families: Vec<QueueFamily> = unsafe { instance.raw.get_physical_device_queue_family_properties(pd) }
                .into_iter()
                .enumerate()
                .map(|(index, properties)| QueueFamily {
                    index: index as u32,
                    properties,
                })
                .collect();

            PhysicalDevice { 
                raw: pd,
                instance: instance.clone(),
                queue_families, 
                features, 
                properties, 
                memory_properties 
            }
        })
        .collect();

    physical_devices
}

pub fn pick_suitable_physical_device(
    instance: &Arc<Instance>,
    surface: &Surface,
) -> PhysicalDevice {
    // NOT support multiple GPUs for now!
    let physical_devices = enumerate_physical_devices(&instance);

    let device: Vec<_> = physical_devices.into_iter()
        .filter(|device| {
            // check if this physical device supports presentation
            device.queue_families.iter()
                .any(|queue| {
                    queue.properties.queue_count > 0 &&
                    queue.properties.queue_flags.contains(vk::QueueFlags::GRAPHICS) &&
                    unsafe { surface.func_loader.get_physical_device_surface_support(device.raw, queue.index, surface.raw).unwrap() }
                })
        })
        .collect();

    glog::trace!("All available physical devices:");
    glog::trace!("{:#?}", device.iter()
        .map(|device| {
            unsafe {
                std::ffi::CStr::from_ptr(device.properties.device_name.as_ptr() as *const std::os::raw::c_char)
            }
        })
        .collect::<Vec<_>>()
    );

    device.into_iter()
        .max_by_key(|device| {
            match device.properties.device_type {
                vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 100,
                vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
                _ => 0,
            }
        }).expect("Failed to find at least one suitable physical device!")
}