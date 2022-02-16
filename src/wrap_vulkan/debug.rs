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
    pub loader: DebugUtils,
    pub messenger: DebugUtilsMessengerEXT,
}

impl Debug {
    pub fn info() -> DebugUtilsMessengerCreateInfoEXT {
        DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | DebugUtilsMessageSeverityFlagsEXT::INFO
                    | DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_utils_callback))
            .build()
    }

    pub fn new(entry: &Entry, instance: &Instance) -> Result<Self> {
        let loader = DebugUtils::new(entry, instance);
        let messenger = unsafe { loader.create_debug_utils_messenger(&Self::info(), None) }?;

        Ok(Self { loader, messenger })
    }
}

impl Drop for Debug {
    fn drop(&mut self) {
        unsafe {
            self.loader
                .destroy_debug_utils_messenger(self.messenger, None)
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
