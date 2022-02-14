use std::ffi::{CStr, CString};

use anyhow::{bail, Error, Result};
use ash::{
    extensions::ext::DebugUtils,
    vk::{
        api_version_major, api_version_minor, make_api_version, ApplicationInfo, Handle,
        InstanceCreateInfo, PhysicalDevice, QueueFlags,
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

pub struct State {
    #[cfg(feature = "validation_vulkan")]
    debug: Debug,

    entry: Entry,
    instance: Instance,
}

impl State {
    pub fn new(xr_base: &wrap_openxr::State) -> Result<State> {
        const VALIDATION_LAYER_NAME: &'static str = "VK_LAYER_KHRONOS_validation";

        log::info!("Creating new Vulkan State");

        let vk_target_version = make_api_version(0, 1, 1, 0); // seems good enough for now

        let reqs = xr_base.get_graphics_requirements()?;
        let xr_vk_target_version = openxr::Version::new(
            api_version_major(vk_target_version) as u16,
            api_version_minor(vk_target_version) as u16,
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

        // this debug marker extension is now part of debug utils and isn't supported by my card
        let device_extensions: Vec<CString> = xr_base
            .get_device_extensions()?
            .into_iter()
            .filter(|ext| *ext != CString::new("VK_EXT_debug_marker").unwrap())
            .collect();

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

        #[cfg(feature = "validation_vulkan")]
        let debug = Debug::new(&entry, &instance)?;

        let physical_device_enumeration = unsafe { instance.enumerate_physical_devices() }?;
        for (i, physical_device) in physical_device_enumeration.iter().enumerate() {
            log::info!("Available physical device nr. {}: {:?}", i, unsafe {
                CStr::from_ptr(
                    instance
                        .get_physical_device_properties(*physical_device)
                        .device_name
                        .as_ptr(),
                )
            });
        }

        // leverage OpenXR to choose for us
        let physical_device =
            PhysicalDevice::from_raw(xr_base.get_physical_device(instance.handle().as_raw())?);

        let device_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device) }?;
        for prop in &device_properties {
            log::trace!("{:?}", unsafe {
                CStr::from_ptr(prop.extension_name.as_ptr())
            });
        }

        for req_ext in &device_extensions {
            if device_properties
                .iter()
                .find(|prop| unsafe { CStr::from_ptr(prop.extension_name.as_ptr()) } == req_ext.as_c_str())
                .is_none()
            {
                bail!("Physical device doesn't support extension: {:?}", req_ext);
            }
        }
        let physical_device_properties =
            unsafe { instance.get_physical_device_properties(physical_device) };
        if physical_device_properties.api_version < vk_target_version {
            unsafe { instance.destroy_instance(None) };
            bail!("Vulkan phyiscal device doesn't support version");
        }

        let queue_family_index =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
                .into_iter()
                .enumerate()
                .filter_map(|(queue_family_index, info)| {
                    log::trace!(
                "{}: GRAPHICS: {:?}, COMPUTE: {:?}, TRANSFER: {:?}, SPARSE_BINDING: {:?}, #: {}",
                queue_family_index,
                info.queue_flags.contains(QueueFlags::GRAPHICS),
                info.queue_flags.contains(QueueFlags::COMPUTE),
                info.queue_flags.contains(QueueFlags::TRANSFER),
                info.queue_flags.contains(QueueFlags::SPARSE_BINDING),
                info.queue_count,
            );

                    if info
                        .queue_flags
                        .contains(QueueFlags::GRAPHICS | QueueFlags::TRANSFER)
                    {
                        Some(queue_family_index as u32)
                    } else {
                        None
                    }
                })
                .last() // this is to log each
                .ok_or(Error::msg("Vulkan device has no suitable queue"))?;
        log::trace!("Using queue nr. {}", queue_family_index);

        Ok(Self {
            #[cfg(feature = "validation_vulkan")]
            debug,

            entry,
            instance,
        })
    }
}
