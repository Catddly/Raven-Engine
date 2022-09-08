use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use std::os::raw::c_char;
use std::collections::HashSet;

use ash::{vk, extensions::khr};

use crate::backend::vulkan::allocator::{Allocator, AllocatorCreateDesc, AllocatorDebugSettings};
use crate::backend::vulkan::buffer::BufferDesc;
use crate::backend::vulkan::{Instance, PhysicalDevice};
use crate::backend::vulkan::util::utility;
use crate::backend::vulkan::constants;

use super::physical_device::QueueFamily;
use super::buffer::Buffer;

/// Descriptor count to subtract from the max bindless descriptor count,
/// so that we don't overflow the max when using bindless _and_ non-bindless descriptors
/// in the same shader stage.
pub const RESERVED_DESCRIPTOR_COUNT: u32 = 32;

pub struct Queue {
    raw: vk::Queue,
    family: QueueFamily,
}

pub struct Device {
    pub raw: ash::Device,
    // since instance and physical device are only valid if and only if device is valid,
    // so keep a atomic reference counter here to avoid incorrect dropping.
    // for convenience purpose, too.
    // aka. Aggregate Design
    pub(crate) physical_device: Arc<PhysicalDevice>,
    pub(crate) instance: Arc<Instance>,
    pub global_allocator: Mutex<Allocator>,
    pub global_queue: Queue,

    pub(crate) crash_tracing_buffer: Buffer,
}

impl Device {
    pub fn builder() -> DeviceBuilder {
        DeviceBuilder::default()
    }

    fn check_extensions_supported(required_extensions: &Vec<&'static CStr>, device_extensions: &HashSet<String>) -> bool {
        required_extensions.iter()
            .all(|ext| {
                let ext = &*ext.to_str().unwrap();
                if !device_extensions.contains(ext) {
                    panic!("Vulkan Extension {} not supported!", ext);
                }
                true
            })
    }

    // check if the graphic card supoort hardware raytracing
    fn check_support_raytracing(extensions: &Vec<&'static CStr>, device_extensions: &HashSet<String>) -> bool {
        extensions.iter()
            .all(|ext| {
                let ext = &*ext.to_str().unwrap();
                let supported = device_extensions.contains(ext);
                if !supported {
                    log::warn!("Graphic card do not support vulkan ray tracing extension: {}", ext);
                }
                supported
            })
    }

    fn populate_device_queue_create_info(physical_device: &Arc<PhysicalDevice>) -> (Vec<vk::DeviceQueueCreateInfo>, QueueFamily) {
        // find a graphic queue
        let graphic_queue = physical_device.queue_families
            .iter()
            .filter(|qf| qf.properties.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .copied()
            .next();
        let graphic_queue = if let Some(graphic_queue) = graphic_queue {
            graphic_queue
        } else {
            panic!("No suitable graphic queue!");
        };

        let priorities = [1.0];

        (vec![
            vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(graphic_queue.index)
            .queue_priorities(&priorities)
            .build()
        ], graphic_queue)
    }

    fn required_layers() -> Vec<CString> {
        let mut layers = Vec::new();
        if constants::ENABLE_DEBUG {
            let raw_layers = constants::REQUIRED_VALIDATION_LAYERS.iter()
                .map(|s| CString::new(*s).unwrap());
            layers.extend(raw_layers);
        }
        layers
    }

    fn new(builder: DeviceBuilder, physical_device: &Arc<PhysicalDevice>) -> anyhow::Result<Self> {
        // get supported device extensions
        let device_extensions = unsafe { physical_device.instance.raw
            .enumerate_device_extension_properties(physical_device.raw) }.expect("Failed to enumerate device extensions!");
        
        let device_extensions: HashSet<String> = device_extensions.into_iter()
            .map(|ext| {
                utility::vk_to_string(&ext.extension_name as &[c_char])
            })
            .collect();

        //glog::trace!("Device supported extensions: {:#?}", device_extensions);

        let mut required_extensions = vec![
            vk::KhrMaintenance1Fn::name(),
            vk::KhrMaintenance2Fn::name(),
            vk::KhrMaintenance3Fn::name(),

            //vk::KhrMaintenance4Fn::name(),
            khr::Swapchain::name(),
        ];
        required_extensions.extend(builder.required_extensions.iter());

        let raytracing_extensions = vec![
            vk::KhrBufferDeviceAddressFn::name(),
            vk::KhrAccelerationStructureFn::name(),
            vk::KhrRayTracingPipelineFn::name(),
        ];

        // if user use ray_tracing features and gpu supports raytracing, add necessary extensions into required extensions
        if constants::ENABLE_GPU_RAY_TRACING && Self::check_support_raytracing(&raytracing_extensions, &device_extensions) {
            required_extensions.extend(raytracing_extensions.iter());
            glog::trace!("GPU Ray Tracing feature enable!");
        }

        // this function will panic if any extension is not supported
        Self::check_extensions_supported(&required_extensions, &device_extensions);

        let required_extensions: Vec<*const c_char> = required_extensions.into_iter()
            .map(|ext| { ext.as_ptr() as *const c_char })
            .collect();

        let (queue_ci, graphic_queue_family) = Self::populate_device_queue_create_info(&physical_device);

        // enable validation for device
        let required_layers = Self::required_layers();
        let required_layers: Vec<*const c_char> = required_layers.iter()
            .map(|layer| layer.as_ptr())
            .collect();

        let mut buffer_device_address_feature = ash::vk::PhysicalDeviceBufferDeviceAddressFeatures::default();

        let mut features2 = vk::PhysicalDeviceFeatures2::builder()
            .push_next(&mut buffer_device_address_feature)
            .build();

        // get device features
        unsafe {
            physical_device.instance.raw.get_physical_device_features2(physical_device.raw, &mut features2);
        }

        // create devices
        let device_ci = vk::DeviceCreateInfo::builder()
            .enabled_layer_names(&required_layers)
            .enabled_extension_names(&required_extensions)
            .queue_create_infos(&queue_ci)
            .push_next(&mut features2)
            .build();

        let device = unsafe { 
            physical_device
            .instance.raw
            .create_device(physical_device.raw, &device_ci, None)
            .expect("Failed to create vulkan device!")
        };
        // once we have the device, we can create gpu resources!
        glog::trace!("Vulkan device created!");

        // create a global gpu memory allocator
        let allocator_debug_settings = AllocatorDebugSettings {
            log_memory_information: true,
            log_allocations: true,
            log_leaks_on_shutdown: true,
            ..Default::default()
        };

        // if constants::ENABLE_DEBUG {
        //     allocator_debug_settings.store_stack_traces = true;
        //     allocator_debug_settings.log_stack_traces = true;
        // }

        let mut global_allocator = Allocator::new(&AllocatorCreateDesc {
            instance: physical_device.instance.raw.clone(),
            device: device.clone(),
            physical_device: physical_device.raw,
            debug_settings: allocator_debug_settings,
            buffer_device_address: true,
        })
        .expect("Failed to create vulkan memory allocator!");

        let global_queue = Queue {
            raw: unsafe { device.get_device_queue(graphic_queue_family.index, 0) },
            family: graphic_queue_family,
        };

        // create crash tracking buffer
        let crash_tracing_buffer = Self::create_buffer_internal(
            &device, 
            &mut global_allocator, 
            BufferDesc::new_gpu_to_cpu(4, vk::BufferUsageFlags::TRANSFER_DST),
            "crash_tracking_buffer"
        )?;

        Ok(Self {
            raw: device,
            physical_device: physical_device.clone(),
            instance: physical_device.instance.clone(),
            global_allocator: Mutex::new(global_allocator),
            global_queue,

            crash_tracing_buffer,
        })
    }

    pub fn max_bindless_descriptor_count(&self) -> u32 {
        (256 * 1024).min(
            self.physical_device
                .properties
                .limits
                .max_per_stage_descriptor_sampled_images
                - RESERVED_DESCRIPTOR_COUNT
        )
    }
}

pub struct DeviceBuilder {
    required_extensions: Vec<&'static CStr>,
}

impl Default for DeviceBuilder {
    fn default() -> Self {
        Self {
            required_extensions: Vec::new(),
        }
    }
}

impl DeviceBuilder {
    #[allow(dead_code)]
    pub fn require_extensions(mut self, extensions: Vec<&'static CStr>) -> Self {
        self.required_extensions = extensions;
        self
    }

    pub fn build(self, physical_device: &Arc<PhysicalDevice>) -> anyhow::Result<Arc<Device>> {
        Ok(Arc::new(Device::new(self, &physical_device)?))
    }
}