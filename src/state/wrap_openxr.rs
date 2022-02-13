use anyhow::Result;
use openxr::{ApplicationInfo, Entry, ExtensionSet, Instance};

#[cfg(feature = "validation_openxr")]
mod debug {
    use anyhow::Result;
    use openxr::{
        raw::DebugUtilsEXT,
        sys::{
            Bool32, DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
            DebugUtilsMessengerEXT,
        },
        DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT, Entry, Instance,
        StructureType,
    };

    pub struct Debug {
        pub debug_utils_loader: DebugUtilsEXT,
        pub debug_messenger: DebugUtilsMessengerEXT,
    }
    impl Debug {
        pub fn new(entry: &Entry, instance: &Instance) -> Result<Self> {
            let debug_utils_loader = unsafe { DebugUtilsEXT::load(&entry, instance.as_raw()) }?;
            let info = DebugUtilsMessengerCreateInfoEXT {
                ty: StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
                next: std::ptr::null(),
                message_severities: DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | DebugUtilsMessageSeverityFlagsEXT::INFO
                    | DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | DebugUtilsMessageSeverityFlagsEXT::ERROR,
                message_types: DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | DebugUtilsMessageTypeFlagsEXT::CONFORMANCE,

                user_callback: Some(openxr_debug_utils_callback),
                user_data: std::ptr::null_mut(),
            };
            let mut debug_messenger = DebugUtilsMessengerEXT::NULL;
            unsafe {
                (debug_utils_loader.create_debug_utils_messenger)(
                    instance.as_raw(),
                    &info,
                    &mut debug_messenger,
                )
            };
            Ok(Self {
                debug_utils_loader,
                debug_messenger,
            })
        }
    }

    impl Drop for Debug {
        fn drop(&mut self) {
            // not going to check that result
            let _ = unsafe { (self.debug_utils_loader.destroy_debug_utils_messenger)(self.debug_messenger) };
        }
    }

    unsafe extern "system" fn openxr_debug_utils_callback(
        message_severity: DebugUtilsMessageSeverityFlagsEXT,
        message_type: DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
        _p_user_data: *mut std::ffi::c_void,
    ) -> Bool32 {
        let type_string = match message_type {
            DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
            DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
            DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
            _ => "[Unknown]",
        };
        let message = std::ffi::CStr::from_ptr((*p_callback_data).message)
            .to_str()
            .unwrap();

        match message_severity {
            DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
                log::debug!("{} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("{} {}", type_string, message),
            DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                log::warn!("{} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{} {}", type_string, message),
            _ => {}
        };
        false.into()
    }
}

#[cfg(feature = "validation_openxr")]
use debug::Debug;

pub struct State {
    #[cfg(feature = "validation_openxr")]
    pub debug: Debug,

    pub entry: Entry,
    pub instance: Instance,
    //pub system_id: SystemId,
    //pub vk_fns: VulkanEnableKHR,
}

impl State {
    pub fn new() -> Result<State> {
        const VALIDATION_LAYER_NAME: &'static str = "XR_APILAYER_LUNARG_core_validation";

        log::info!("Creating new OpenXR State");

        let entry = Entry::linked();
        let available_extensions = entry.enumerate_extensions()?;
        let available_layers = entry.enumerate_layers()?;

        log::trace!("available_extensions: {:?}", available_extensions);
        log::trace!("available_layers: {:?}", available_layers);

        assert!(available_extensions.khr_vulkan_enable);

        #[cfg(feature = "validation_openxr")]
        assert!(
            available_layers
                .iter()
                .find(|l| l.layer_name == VALIDATION_LAYER_NAME)
                .is_some(),
            "Validation layer not found, did you set XR_API_LAYER_PATH?"
        );

        let mut enabled_extensions = ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable = true;
        if cfg!(feature = "validation_openxr") {
            enabled_extensions.ext_debug_utils = true;
        }
        let instance = entry.create_instance(
            &ApplicationInfo {
                application_name: "VRV App",
                application_version: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
                engine_name: "",
                engine_version: 0,
            },
            &enabled_extensions,
            if cfg!(feature = "validation_openxr") {
                &[VALIDATION_LAYER_NAME]
            } else {
                &[]
            },
        )?;

        #[cfg(feature = "validation_openxr")]
        let debug = Debug::new(&entry, &instance)?;

        Ok(State {
            #[cfg(feature = "validation_openxr")]
            debug,

            entry,
            instance,
            //system_id,
            //vk_fns,
        })
    }
}
