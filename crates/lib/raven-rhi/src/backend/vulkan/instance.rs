use std::ffi::{CStr, CString};
use std::sync::Arc;

use ash::vk;

use super::constants;
use super::debug;
use super::platform;

pub struct Instance {
    pub(crate) entry: ash::Entry, // should entry be here? 
    pub raw: ash::Instance,
}

impl Instance {
    pub fn builder() -> InstanceBuilder {
        InstanceBuilder::default()
    }

    fn new(builder: InstanceBuilder) -> anyhow::Result<Self> {
        // load vulkan dll
        //let entry = unsafe { ash::Entry::load()? };
        let entry = unsafe { ash::Entry::new()? };
        
        // if in debug build, check if the validation layer is supported
        if constants::ENABLE_DEBUG && !debug::check_validation_layer_support(&entry, &constants::REQUIRED_VALIDATION_LAYERS.to_vec()) {
            glog::error!("vulkan validation layer not support, but requested!");
            panic!("vulkan validation layer not support, but requested!");
        }

        // create vulkan instance
        let instance = Self::create_instance(&entry, &builder);

        Ok(Self {
            entry,
            raw: instance,
        })
    }

    fn required_layers(builder: &InstanceBuilder) -> Vec<CString> {
        let mut layers = Vec::new();
        if builder.enable_debug {
            let raw_layers = constants::REQUIRED_VALIDATION_LAYERS.iter()
                .map(|s| CString::new(*s).unwrap());
            layers.extend(raw_layers);
        }
        layers
    }

    fn create_instance(
        entry: &ash::Entry, 
        builder: &InstanceBuilder,
    ) -> ash::Instance {
        let app_info = vk::ApplicationInfo::builder()
            .api_version(vk::make_api_version(0, 1, 3, 0))
            .application_name(CString::new("Raven Engine").unwrap().as_c_str())
            .engine_name(CString::new("Raven Vulkan RenderDevice").unwrap().as_c_str())
            .build();

        let mut debug_messenger_create_info = debug::populate_debug_messenger_create_info();

        // all required extensions
        let extension_names: Vec<*const i8> = builder.required_extensions.iter()
            .map(|s| s.as_ptr())
            .chain(platform::required_extension_names().into_iter().map(|n| n.as_ptr()))
            .collect();
        // all required layers
        let layer_names = Self::required_layers(&builder);
        let layer_names: Vec<*const i8> = layer_names
            .iter()
            .map(|raw| raw.as_ptr())
            .collect();
        
        let mut instance_ci_builder = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names)
            .enabled_layer_names(&layer_names);
        if constants::ENABLE_DEBUG {
            instance_ci_builder = instance_ci_builder.push_next(&mut debug_messenger_create_info);
        }
        let create_info = instance_ci_builder.build();

        // create vulkan instance
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .expect("Failed to create vulkan instance!");
        glog::trace!("Vulkan instance created!");

        instance
    }
}

pub struct InstanceBuilder {
    pub required_extensions: Vec<&'static CStr>,
    pub enable_debug: bool,
}

impl Default for InstanceBuilder {
    fn default() -> Self {
        InstanceBuilder {
            required_extensions: Vec::new(),
            enable_debug: true,
        }
    }
}

impl InstanceBuilder {
    #[allow(dead_code)]
    pub fn require_extensions(mut self, extensions: Vec<&'static CStr>) -> Self {
        self.required_extensions = extensions;
        self
    }
    
    #[allow(dead_code)]
    pub fn enable_debug(mut self, enable: bool) -> Self { 
        self.enable_debug = enable;
        self
    }

    pub fn build(self) -> anyhow::Result<Arc<Instance>> {
        Ok(Arc::new(Instance::new(self)?))
    }
}