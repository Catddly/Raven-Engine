use std::cell::Cell;
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::os::raw::c_char;
use std::collections::{HashSet, HashMap};

use parking_lot::Mutex;
use ash::{vk, extensions::khr};

use crate::backend::{CommandBuffer, DEVICE_DRAW_FRAMES};
use crate::backend::vulkan::allocator::{Allocator, AllocatorCreateDesc, AllocatorDebugSettings};
use crate::backend::vulkan::buffer::BufferDesc;
use crate::backend::vulkan::{Instance, PhysicalDevice};
use crate::backend::vulkan::utility;
use crate::backend::vulkan::constants;
use crate::draw_frame::{DrawFrame, DeferReleasableResource};

use super::RhiError;
use super::physical_device::QueueFamily;
use super::buffer::Buffer;
use super::sampler::SamplerDesc;

/// Descriptor count to subtract from the max bindless descriptor count,
/// so that we don't overflow the max when using bindless _and_ non-bindless descriptors
/// in the same shader stage.
pub const RESERVED_DESCRIPTOR_COUNT: u32 = 32;

pub struct Queue {
    pub raw: vk::Queue,
    pub family: QueueFamily,
}

#[cfg(feature = "gpu_ray_tracing")]
pub struct RayTracingExts {
    pub acceleration_structure_khr: khr::AccelerationStructure,
    pub ray_tracing_pipeline_khr: khr::RayTracingPipeline,
    pub acceleration_structure_props: vk::PhysicalDeviceAccelerationStructurePropertiesKHR,
    pub ray_tracing_props: vk::PhysicalDeviceRayTracingPipelinePropertiesKHR,
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

    pub(crate) immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,

    pub(crate) crash_tracing_buffer: Cell<Option<Buffer>>,
    setup_cb: Mutex<CommandBuffer>,

    #[cfg(feature = "gpu_ray_tracing")]
    pub ray_tracing_extensions: RayTracingExts,

    ray_tracing_enabled: bool,
    current_frame: Cell<u32>,
    // CPU frames.
    // Note: In CPU controller side, we only have 2 frames here. But in the swapchain we have 3 images.
    draw_frames: [Mutex<Arc<DrawFrame>>; DEVICE_DRAW_FRAMES],
}

impl Device {
    pub fn builder() -> DeviceBuilder {
        DeviceBuilder::default()
    }

    pub fn wait_idle(&self) {
        unsafe {
            self.raw.device_wait_idle().expect("Failed to wait device idle!");
        }
    }

    pub fn defer_release(&self, resource: impl DeferReleasableResource) {
        let current_frame = self.current_frame.get() as usize;
        let draw_frame = self.draw_frames[current_frame].lock();
        let mut defer_release_resources = draw_frame.defer_release_resources.lock();
        
        resource.enqueue(&mut defer_release_resources);
    }

    pub fn get_device_frame_index(&self) -> u32 {
        self.current_frame.get()
    }

    pub fn begin_frame(&self) -> Arc<DrawFrame> {
        let current_frame = self.current_frame.get() as usize;
        let mut draw_frame = &mut self.draw_frames[current_frame].lock();

        // make sure user can NOT modify this frame anymore
        match Arc::get_mut(&mut draw_frame) {
            Some(frame) => {
                // wait for current frame to be submitted in the GPU-side, or we may change the command buffer while GPU is submitting.
                unsafe {
                    self.raw
                        .wait_for_fences(&[
                            frame.main_command_buffer.submit_done_fence,
                            frame.present_command_buffer.submit_done_fence
                        ], true, std::u64::MAX)
                        .unwrap();
                }
            },
            None => panic!("User-side is still using DrawFrame data!"),
        };

        // release previous frame's stale resources
        draw_frame.release_stale_render_resources(self);
        draw_frame.clone()
    }

    pub fn end_frame(&self, frame: Arc<DrawFrame>) {
        drop(frame);

        let mut current_frame = self.current_frame.get() as usize;

        // check again to make sure no one is modifying this frame.
        Arc::get_mut(&mut self.draw_frames[current_frame].lock())
            .unwrap_or_else(|| panic!("Failed to end this frame! User still holding this frame data!"));

        // advance to next frame
        current_frame = (current_frame + 1) % self.draw_frames.len();
        self.current_frame.set(current_frame as u32);
    }

    pub fn is_ray_tracing_enabled(&self) -> bool {
        self.ray_tracing_enabled
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

            vk::KhrSamplerMirrorClampToEdgeFn::name(),

            //vk::KhrMaintenance4Fn::name(),
            vk::KhrBufferDeviceAddressFn::name(),

            khr::Swapchain::name(),
        ];
        required_extensions.extend(builder.required_extensions.iter());

        let mut ray_tracing_enabled = false;
        let raytracing_extensions = vec![
            vk::KhrAccelerationStructureFn::name(),  // required to build acceleration structures
            vk::KhrRayTracingPipelineFn::name(),     // required to use vkCmdTraceRaysKHR
            vk::KhrDeferredHostOperationsFn::name(), // required by ray tracing pipeline

            vk::KhrRayQueryFn::name(),
        ];

        // if user use ray_tracing features and gpu supports raytracing, add necessary extensions into required extensions
        if constants::ENABLE_GPU_RAY_TRACING && Self::check_support_raytracing(&raytracing_extensions, &device_extensions) {
            required_extensions.extend(raytracing_extensions.iter());
            glog::trace!("GPU Ray Tracing feature enable!");
            ray_tracing_enabled = true;
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

        let mut buffer_device_address_feature = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
        let mut descriptor_indexing = vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::default();
        let mut imageless_framebuffer = vk::PhysicalDeviceImagelessFramebufferFeaturesKHR::default();

        let mut ray_tracing_pipeline_feature = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
        let mut accel_struct_feature = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut ray_query_feature = vk::PhysicalDeviceRayQueryFeaturesKHR::builder().ray_query(true).build();

        let mut features2 = if ray_tracing_enabled {
            vk::PhysicalDeviceFeatures2::builder()
                .push_next(&mut ray_query_feature)
                .push_next(&mut ray_tracing_pipeline_feature)
                .push_next(&mut accel_struct_feature)
                .push_next(&mut buffer_device_address_feature)
                .push_next(&mut descriptor_indexing)
                .push_next(&mut imageless_framebuffer)
                .build()
        } else {
            vk::PhysicalDeviceFeatures2::builder()
                .push_next(&mut buffer_device_address_feature)
                .push_next(&mut descriptor_indexing)
                .push_next(&mut imageless_framebuffer)
                .build()
        };

        // query device features
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

        let draw_frames = [
            Mutex::new(Arc::new(DrawFrame::new(&device, &global_queue.family))),
            Mutex::new(Arc::new(DrawFrame::new(&device, &global_queue.family)))
        ];

        let setup_cb = Mutex::new(CommandBuffer::new(&device, &global_queue.family));
        let immutable_samplers = Self::create_immutable_samplers(&device);

        #[cfg(feature = "gpu_ray_tracing")]
        let ray_tracing_extensions = {
            let acceleration_structure_ext =
                khr::AccelerationStructure::new(&physical_device.instance.raw, &device);
            let ray_tracing_pipeline_ext =
                khr::RayTracingPipeline::new(&physical_device.instance.raw, &device);
            let ray_tracing_pipeline_properties = unsafe {
                khr::RayTracingPipeline::get_properties(&physical_device.instance.raw, physical_device.raw)
            };
   
            // fetch physical device ray tracing props 
            let mut as_props = vk::PhysicalDeviceAccelerationStructurePropertiesKHR::default();
            let mut properties_2 = vk::PhysicalDeviceProperties2::builder()
                .push_next(&mut as_props)
                .build();
                
            unsafe { 
                physical_device.instance.raw.get_physical_device_properties2(physical_device.raw, &mut properties_2)
            };

            RayTracingExts { 
                acceleration_structure_khr: acceleration_structure_ext, 
                ray_tracing_pipeline_khr: ray_tracing_pipeline_ext, 
                acceleration_structure_props: as_props,
                ray_tracing_props: ray_tracing_pipeline_properties,
            }
        };

        Ok(Self {
            raw: device,
            physical_device: physical_device.clone(),
            instance: physical_device.instance.clone(),
            global_allocator: Mutex::new(global_allocator),
            global_queue,

            immutable_samplers,

            crash_tracing_buffer: Cell::new(Some(crash_tracing_buffer)),
            setup_cb,

            #[cfg(feature = "gpu_ray_tracing")]
            ray_tracing_extensions,

            ray_tracing_enabled,
            current_frame: Cell::new(0),
            draw_frames,
        })
    }

    pub fn max_bindless_descriptor_count(&self) -> u32 {
        (512 * 1024).min(
            self.physical_device
                .properties
                .limits
                .max_per_stage_descriptor_sampled_images
                - RESERVED_DESCRIPTOR_COUNT
        )
    }

    pub(crate) fn release_debug_resources(&self) {
        if let Some(crash_tracking_buffer) = self.crash_tracing_buffer.take() {
            self.destroy_buffer(crash_tracking_buffer);
        }
    }

    pub fn with_setup_commands(&self, callback: impl FnOnce(vk::CommandBuffer)) -> anyhow::Result<(), RhiError> {
        let cb = &self.setup_cb.lock();

        unsafe {
            self.raw
                .begin_command_buffer(
                    cb.raw,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
        }

        callback(cb.raw);

        unsafe {
            self.raw.end_command_buffer(cb.raw).unwrap();

            let submit_info = vk::SubmitInfo::builder().command_buffers(std::slice::from_ref(&cb.raw));

            self.raw
                .queue_submit(
                    self.global_queue.raw,
                    &[submit_info.build()],
                    vk::Fence::null(),
                )
                .expect("Failed to submit setup commands to global queue!");

            // TODO: use copy queue and render graph dependencies
            self.raw.device_wait_idle()?;
        }

        Ok(())
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