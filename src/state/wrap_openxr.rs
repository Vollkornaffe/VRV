use std::{
    ffi::{CStr, CString},
    mem,
    os::raw::c_char,
    ptr::null_mut,
};

use anyhow::{bail, Result};
use ash::vk::{self, Handle};
use openxr::{
    raw::VulkanEnableKHR,
    sys::{
        self,
        platform::{VkInstance, VkPhysicalDevice},
        GraphicsRequirementsVulkanKHR,
    },
    ApplicationInfo, Entry, EnvironmentBlendMode, ExtensionSet, FormFactor, Instance,
    StructureType, SystemId, Version, ViewConfigurationType, Vulkan, vulkan::Requirements,
};

fn check(instance: &Instance, xr_result: sys::Result) -> Result<()> {
    if xr_result != sys::Result::SUCCESS {
        bail!("{}", instance.result_to_string(xr_result).unwrap());
    }
    Ok(())
}

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

    use super::check;

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
            check(instance, unsafe {
                (debug_utils_loader.create_debug_utils_messenger)(
                    instance.as_raw(),
                    &info,
                    &mut debug_messenger,
                )
            })?;
            Ok(Self {
                debug_utils_loader,
                debug_messenger,
            })
        }
    }

    impl Drop for Debug {
        fn drop(&mut self) {
            // not going to check that result
            let _ = unsafe {
                (self.debug_utils_loader.destroy_debug_utils_messenger)(self.debug_messenger)
            };
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
                log::debug!("OPENXR: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::INFO => {
                log::info!("OPENXR: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                log::warn!("OPENXR: {} {}", type_string, message)
            }
            DebugUtilsMessageSeverityFlagsEXT::ERROR => {
                log::error!("OPENXR: {} {}", type_string, message)
            }
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
    pub system_id: SystemId,
    pub vk_fns: VulkanEnableKHR,
}

impl State {
    pub fn new() -> Result<State> {
        const VALIDATION_LAYER_NAME: &'static str = "XR_APILAYER_LUNARG_core_validation";

        log::info!("Creating new OpenXR State");

        let entry = Entry::linked();
        let available_extensions = entry.enumerate_extensions()?;
        let available_layers = entry.enumerate_layers()?;

        log::trace!("OpenXR available extensions: {:?}", available_extensions);
        log::trace!("OpenXR available layers: {:?}", available_layers);

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

        let instance_props = instance.properties()?;
        log::info!(
            "loaded OpenXR runtime: {} {}",
            instance_props.runtime_name,
            instance_props.runtime_version
        );

        // Request a form factor from the device (HMD, Handheld, etc.)
        let system_id = instance.system(FormFactor::HEAD_MOUNTED_DISPLAY)?;
        if instance
            .enumerate_environment_blend_modes(system_id, ViewConfigurationType::PRIMARY_STEREO)?
            .into_iter()
            .find(|&mode| mode == EnvironmentBlendMode::OPAQUE)
            == None
        {
            bail!("Only OPAQUE mode allowed");
        }

        let vk_fns = unsafe { VulkanEnableKHR::load(&entry, instance.as_raw()) }?;

        Ok(State {
            #[cfg(feature = "validation_openxr")]
            debug,

            entry,
            instance,
            system_id,
            vk_fns,
        })
    }

    pub fn get_graphics_requirements(&self) -> Result<GraphicsRequirementsVulkanKHR> {
        let mut graphics_requirements: GraphicsRequirementsVulkanKHR = unsafe { mem::zeroed() };
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_graphics_requirements)(
                self.instance.as_raw(),
                self.system_id,
                &mut graphics_requirements,
            )
        })?;
        Ok(graphics_requirements)
    }

    pub fn get_instance_extensions(&self) -> Result<Vec<CString>> {
        let mut count: u32 = unsafe { mem::zeroed() };
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_instance_extensions)(
                self.instance.as_raw(),
                self.system_id,
                0,
                &mut count,
                null_mut::<c_char>(),
            )
        })?;
        let mut extensions_chars = Vec::<c_char>::with_capacity(count as usize);
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_instance_extensions)(
                self.instance.as_raw(),
                self.system_id,
                count,
                &mut count,
                extensions_chars.as_mut_ptr(),
            )
        })?;
        let result: Result<_, _> = unsafe {
            CStr::from_ptr(extensions_chars.as_ptr())
                .to_str()?
                .rsplit(' ')
                .map(|s| CString::new(s))
                .collect()
        };
        Ok(result?)
    }

    pub fn get_device_extensions(&self) -> Result<Vec<CString>> {
        let mut count: u32 = unsafe { mem::zeroed() };
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_device_extensions)(
                self.instance.as_raw(),
                self.system_id,
                0,
                &mut count,
                null_mut::<c_char>(),
            )
        })?;
        let mut extensions_chars = Vec::<c_char>::with_capacity(count as usize);
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_device_extensions)(
                self.instance.as_raw(),
                self.system_id,
                count,
                &mut count,
                extensions_chars.as_mut_ptr(),
            )
        })?;
        let result: Result<_, _> = unsafe {
            CStr::from_ptr(extensions_chars.as_ptr())
                .to_str()?
                .rsplit(' ')
                .map(|s| CString::new(s))
                .collect()
        };
        Ok(result?)
    }

    pub fn get_physical_device(&self, vk_instance: vk::Instance) -> Result<vk::PhysicalDevice> {
        let mut vk_physical_device: VkPhysicalDevice = unsafe { mem::zeroed() };
        check(&self.instance, unsafe {
            (self.vk_fns.get_vulkan_graphics_device)(
                self.instance.as_raw(),
                self.system_id,
                vk_instance.as_raw() as VkInstance,
                &mut vk_physical_device,
            )
        })?;
        Ok(vk::PhysicalDevice::from_raw(vk_physical_device as u64))
    }
}
