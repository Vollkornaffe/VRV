use std::ffi::{CStr, CString};

use anyhow::{bail, Result};
use ash::{
    extensions::ext::DebugUtils,
    vk::{
        make_version, version_major, version_minor, ApplicationInfo, InstanceCreateInfo,
        ValidationCacheCreateInfoEXT,
    },
    Entry, Instance,
};

use super::wrap_openxr;

#[cfg(feature = "validation_vulkan")]
mod debug {
    use anyhow::Result;
    use ash::{
        extensions::ext::DebugUtils,
        vk::{
            Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
            DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
            DebugUtilsMessengerEXT, FALSE,
        },
        Entry, Instance,
    };

    pub struct Debug {
        debug_utils_loader: DebugUtils,
        debug_messenger: DebugUtilsMessengerEXT,
    }

    impl Debug {
        pub fn info() -> DebugUtilsMessengerCreateInfoEXT {
            DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                        | DebugUtilsMessageSeverityFlagsEXT::INFO
                        | DebugUtilsMessageSeverityFlagsEXT::ERROR,
                )
                .message_type(
                    DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                        | DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                )
                .pfn_user_callback(Some(vulkan_debug_utils_callback))
                .build()
        }

        pub fn new(entry: &Entry, instance: &Instance) -> Result<Self> {
            let debug_utils_loader = DebugUtils::new(entry, instance);
            let debug_messenger =
                unsafe { debug_utils_loader.create_debug_utils_messenger(&Self::info(), None) }?;

            Ok(Self {
                debug_utils_loader,
                debug_messenger,
            })
        }
    }

    impl Drop for Debug {
        fn drop(&mut self) {
            unsafe {
                self.debug_utils_loader
                    .destroy_debug_utils_messenger(self.debug_messenger, None)
            }
        }
    }

    unsafe extern "system" fn vulkan_debug_utils_callback(
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
        let message = std::ffi::CStr::from_ptr((*p_callback_data).p_message)
            .to_str()
            .unwrap();

        match message_severity {
            DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
                log::debug!("VULKAN: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                log::warn!("VULKAN: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::ERROR => {
                log::error!("VULKAN: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::INFO => {
                log::info!("VULKAN: {} {}", type_string, message)
            }
            _ => {}
        };
        FALSE
    }
}

#[cfg(feature = "validation_vulkan")]
use debug::Debug;

pub struct State {}

impl State {
    pub fn new(xr_base: &wrap_openxr::State) -> Result<State> {
        const VALIDATION_LAYER_NAME: &'static str = "VK_LAYER_KHRONOS_validation";

        log::info!("Creating new Vulkan State");

        let vk_target_version = make_version(1, 2, 0);

        let reqs = xr_base.get_graphics_requirements()?;
        let xr_vk_target_version = openxr::Version::new(
            version_major(vk_target_version) as u16,
            version_minor(vk_target_version) as u16,
            0,
        );

        if reqs.min_api_version_supported > xr_vk_target_version
            || reqs.max_api_version_supported < xr_vk_target_version
        {
            bail!("OpenXR needs other Vulkan version");
        }

        let instance_extensions = xr_base.get_instance_extensions()?;
        #[cfg(feature = "validation_vulkan")]
        let instance_extensions =
            [instance_extensions.as_slice(), &[DebugUtils::name().into()]].concat();

        log::trace!("Vulkan instance extensions: {:?}", instance_extensions);

        let device_extensions = xr_base.get_device_extensions()?;

        log::trace!("Vulkan device extensions: {:?}", device_extensions);

        let entry = unsafe { Entry::load() }?;

        // this pains me :(
        let app_info = ApplicationInfo::builder()
            .api_version(vk_target_version)
            .build();
        let instance_extensions = instance_extensions
            .iter()
            .map(|ext| ext.as_c_str().as_ptr())
            .collect::<Vec<_>>();
        let info = InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_name = CString::new(VALIDATION_LAYER_NAME).unwrap();
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_names = [c_str_layer_name.as_ptr()];
        #[cfg(feature = "validation_vulkan")]
        let mut debug_info = Debug::info();
        #[cfg(feature = "validation_vulkan")]
        let info = info
            .enabled_layer_names(&c_str_layer_names)
            .push_next(&mut debug_info);

        let instance = unsafe { entry.create_instance(&info, None) }?;

        Ok(Self {})
    }
}
