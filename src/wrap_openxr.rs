use std::ffi::CString;

use anyhow::{bail, Result, Error};
use ash::vk::{Extent2D, Format, Handle};
use openxr::{
    raw::VulkanEnableKHR,
    sys,
    vulkan::{self, Requirements, SessionCreateInfo},
    ApplicationInfo, Entry, EnvironmentBlendMode, ExtensionSet, FormFactor, FrameStream,
    FrameWaiter, Instance, Session, Swapchain, SwapchainCreateFlags, SwapchainCreateInfo,
    SwapchainUsageFlags, SystemId, ViewConfigurationType, Vulkan,
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

use crate::wrap_vulkan;

pub struct Base {
    #[cfg(feature = "validation_openxr")]
    pub debug: Debug,

    pub entry: Entry,
    pub instance: Instance,
    pub system_id: SystemId,
    pub vk_fns: VulkanEnableKHR,
}

impl Base {
    pub fn new() -> Result<Self> {
        const VALIDATION_LAYER_NAME: &'static str = "XR_APILAYER_LUNARG_core_validation";

        log::info!("Creating new OpenXR Base");

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

        Ok(Self {
            #[cfg(feature = "validation_openxr")]
            debug,

            entry,
            instance,
            system_id,
            vk_fns,
        })
    }

    pub fn get_graphics_requirements(&self) -> Result<Requirements> {
        Ok(self
            .instance
            .graphics_requirements::<Vulkan>(self.system_id)?)
    }

    pub fn get_instance_extensions(&self) -> Result<Vec<CString>> {
        let result: Result<_, _> = self
            .instance
            .vulkan_legacy_instance_extensions(self.system_id)?
            .split(' ')
            .map(|s| CString::new(s))
            .collect();
        Ok(result?)
    }

    pub fn get_device_extensions(&self) -> Result<Vec<CString>> {
        let result: Result<_, _> = self
            .instance
            .vulkan_legacy_device_extensions(self.system_id)?
            .split(' ')
            .map(|s| CString::new(s))
            // this debug marker extension is now part of debug utils and isn't supported by my card
            .filter(|ext| *ext != CString::new("VK_EXT_debug_marker"))
            .collect();
        Ok(result?)
    }

    // using raw stuff
    pub fn get_physical_device(&self, vk_instance: u64) -> Result<u64> {
        Ok(self
            .instance
            .vulkan_graphics_device(self.system_id, vk_instance as _)? as _)
    }

    pub fn get_resolution(&self) -> Result<Extent2D> {
        let views = self.instance.enumerate_view_configuration_views(
            self.system_id,
            ViewConfigurationType::PRIMARY_STEREO,
        )?;

        if views.len() != 2 {
            bail!("Views are not 2");
        }
        if views[0].recommended_image_rect_width != views[1].recommended_image_rect_width
            || views[0].recommended_image_rect_height != views[1].recommended_image_rect_height
        {
            bail!("Views don't have equal resolution?");
        }

        Ok(Extent2D {
            width: views[0].recommended_image_rect_width,
            height: views[0].recommended_image_rect_height,
        })
    }

    pub fn find_supported_format(
        session: &Session<Vulkan>,
        candidates: &[Format],
    ) -> Result<Format> {
        let supported_formats = session.enumerate_swapchain_formats()?;

        candidates
            .iter()
            .find(|&wanted| supported_formats.iter().find(|&supported| *supported == wanted.as_raw() as u32).is_some())
            .ok_or(Error::msg("Couldn't find supported format"))
            .cloned()
    }

    pub fn get_swapchain(
        session: &Session<Vulkan>,
        resolution: Extent2D,
        format: Format,
    ) -> Result<Swapchain<Vulkan>> {
        Ok(session.create_swapchain(&SwapchainCreateInfo {
            create_flags: SwapchainCreateFlags::EMPTY,
            usage_flags: SwapchainUsageFlags::COLOR_ATTACHMENT | SwapchainUsageFlags::SAMPLED,
            format: format.as_raw() as _,
            sample_count: 1,
            width: resolution.width,
            height: resolution.height,
            face_count: 1,
            array_size: 2, // two eyes
            mip_count: 1,
        })?)
    }

    pub fn init_with_vulkan(
        &self,
        vk_base: &wrap_vulkan::Base,
    ) -> Result<(Session<Vulkan>, FrameWaiter, FrameStream<Vulkan>)> {
        // A session represents this application's desire to display things! This is where we hook
        // up our graphics API. This does not start the session; for that, you'll need a call to Session::begin
        Ok(unsafe {
            self.instance.create_session::<Vulkan>(
                self.system_id,
                &SessionCreateInfo {
                    instance: vk_base.instance.handle().as_raw() as _,
                    physical_device: vk_base.physical_device.as_raw() as _,
                    device: vk_base.device.handle().as_raw() as _,
                    queue_family_index: vk_base.queue_family_index,
                    queue_index: 0,
                },
            )
        }?)
    }


}
