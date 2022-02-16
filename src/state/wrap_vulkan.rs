use std::{
    ffi::{CStr, CString},
    mem::ManuallyDrop,
};

use anyhow::{bail, Error, Result};
use ash::{
    extensions::{ext::DebugUtils, khr::Surface},
    vk::{
        api_version_major, api_version_minor, make_api_version, ApplicationInfo,
        CommandPoolCreateFlags, CommandPoolCreateInfo, DeviceCreateInfo, DeviceQueueCreateInfo,
        Handle, InstanceCreateInfo, PhysicalDevice, PresentModeKHR, QueueFlags,
        SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR,
    },
    Device, Entry, Instance,
};

use super::{wrap_openxr, wrap_window};

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

struct SurfaceRelated {
    pub surface_loader: Surface,
    pub surface: SurfaceKHR,
    pub capabilities: SurfaceCapabilitiesKHR,
    pub formats: Vec<SurfaceFormatKHR>,
    pub present_modes: Vec<PresentModeKHR>,
}

impl Drop for SurfaceRelated {
    fn drop(&mut self) {
        unsafe { self.surface_loader.destroy_surface(self.surface, None) };
    }
}

pub struct State {
    entry: ManuallyDrop<Entry>,
    instance: ManuallyDrop<Instance>,
    physical_device: ManuallyDrop<PhysicalDevice>,
    device: ManuallyDrop<Device>,

    #[cfg(feature = "validation_vulkan")]
    debug: ManuallyDrop<Debug>,

    surface_related: ManuallyDrop<SurfaceRelated>,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self.physical_device);
            ManuallyDrop::drop(&mut self.surface_related);
            #[cfg(feature = "validation_vulkan")]
            ManuallyDrop::drop(&mut self.debug);
            ManuallyDrop::drop(&mut self.instance);
            ManuallyDrop::drop(&mut self.entry);
        }
    }
}

impl State {
    pub fn new(window_state: &wrap_window::State, xr_state: &wrap_openxr::State) -> Result<State> {
        #[cfg(feature = "validation_vulkan")]
        const VALIDATION_LAYER_NAME: &'static str = "VK_LAYER_KHRONOS_validation";
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_name = CString::new(VALIDATION_LAYER_NAME).unwrap();
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_names = [c_str_layer_name.as_ptr()];

        #[cfg(not(feature = "validation_vulkan"))]
        let c_str_layer_names = [];

        log::info!("Creating new Vulkan State");

        let vk_target_version = make_api_version(0, 1, 1, 0); // seems good enough for now

        let reqs = xr_state.get_graphics_requirements()?;
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

        let instance_extensions = [
            window_state.get_instance_extensions()?,
            xr_state.get_instance_extensions()?,
            // hehe sneaky
            #[cfg(feature = "validation_vulkan")]
            vec![DebugUtils::name().into()],
        ]
        .concat();

        log::trace!("Vulkan instance extensions: {:?}", instance_extensions);

        let entry = unsafe { Entry::load() }?;

        #[cfg(feature = "validation_vulkan")]
        let mut debug_info = Debug::info();

        // I really couldn't find a better way to do this
        // the problem is that push_next can't take a "null object"
        let instance = unsafe {
            entry.create_instance(
                #[cfg(feature = "validation_vulkan")]
                &InstanceCreateInfo::builder()
                    .application_info(&ApplicationInfo::builder().api_version(vk_target_version))
                    .enabled_extension_names(
                        &instance_extensions
                            .iter()
                            .map(|ext| ext.as_c_str().as_ptr())
                            .collect::<Vec<_>>(),
                    )
                    .enabled_layer_names(&c_str_layer_names)
                    .push_next(&mut debug_info),
                #[cfg(not(feature = "validation_vulkan"))]
                &InstanceCreateInfo::builder()
                    .application_info(&ApplicationInfo::builder().api_version(vk_target_version))
                    .enabled_extension_names(
                        &instance_extensions
                            .iter()
                            .map(|ext| ext.as_c_str().as_ptr())
                            .collect::<Vec<_>>(),
                    ),
                None,
            )
        }?;

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
            PhysicalDevice::from_raw(xr_state.get_physical_device(instance.handle().as_raw())?);

        let surface_related = {
            let surface_loader = Surface::new(&entry, &instance);
            let surface = unsafe {
                ash_window::create_surface(&entry, &instance, &window_state.window, None)
            }?;
            let capabilities = unsafe {
                surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
            }?;
            let formats = unsafe {
                surface_loader.get_physical_device_surface_formats(physical_device, surface)
            }?;
            let present_modes = unsafe {
                surface_loader.get_physical_device_surface_present_modes(physical_device, surface)
            }?;
            if formats.is_empty() || present_modes.is_empty() {
                bail!("Physical device incompatible with surface")
            }
            SurfaceRelated {
                surface_loader,
                surface,
                capabilities,
                formats,
                present_modes,
            }
        };

        let physical_device_extension_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device) }?;
        for prop in &physical_device_extension_properties {
            log::trace!("{:?}", unsafe {
                CStr::from_ptr(prop.extension_name.as_ptr())
            });
        }

        let device_extensions: Vec<CString> = [
            xr_state.get_device_extensions()?,
            window_state.get_device_extensions(),
        ]
        .concat();

        log::trace!("Vulkan device extensions: {:?}", device_extensions);

        for req_ext in &device_extensions {
            if physical_device_extension_properties
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
            bail!("Vulkan phyiscal device doesn't support target version");
        }

        let queue_family_index =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
                .into_iter()
                .enumerate()
                .map(|(queue_family_index, info)| -> Result<bool> {
                    let supp_graphics = info.queue_flags.contains(QueueFlags::GRAPHICS);
                    //let supp_compute = info.queue_flags.contains(QueueFlags::COMPUTE);
                    let supp_transfer = info.queue_flags.contains(QueueFlags::TRANSFER);
                    //let supp_sparse = info.queue_flags.contains(QueueFlags::SPARSE_BINDING);
                    let supp_present = unsafe {
                        surface_related
                            .surface_loader
                            .get_physical_device_surface_support(
                                physical_device,
                                queue_family_index as u32,
                                surface_related.surface,
                            )
                    }?;
                    Ok(supp_graphics && supp_present && supp_transfer)
                })
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .enumerate()
                .find_map(|(queue_family_index, suitable)| {
                    if *suitable {
                        Some(queue_family_index as u32)
                    } else {
                        None
                    }
                })
                .ok_or(Error::msg("Vulkan device has no suitable queue"))?;

        log::trace!("Using queue nr. {}", queue_family_index);

        let device = unsafe {
            instance.create_device(
                physical_device,
                &DeviceCreateInfo::builder()
                    .queue_create_infos(&[DeviceQueueCreateInfo::builder()
                        .queue_family_index(queue_family_index)
                        .queue_priorities(&[1.0])
                        .build()])
                    .enabled_extension_names(
                        &device_extensions
                            .iter()
                            .map(|ext| ext.as_ptr())
                            .collect::<Vec<_>>(),
                    )
                    .enabled_layer_names(if cfg!(feature = "validation_vulkan") {
                        &c_str_layer_names
                    } else {
                        &[]
                    }),
                None,
            )
        }?;

        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        let command_pool = unsafe {
            device.create_command_pool(
                &CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_family_index)
                    .flags(
                        CommandPoolCreateFlags::TRANSIENT
                            | CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    ),
                None,
            )
        }?;

        Ok(Self {
            entry: ManuallyDrop::new(entry),
            instance: ManuallyDrop::new(instance),
            physical_device: ManuallyDrop::new(physical_device),
            device: ManuallyDrop::new(device),

            #[cfg(feature = "validation_vulkan")]
            debug: ManuallyDrop::new(debug),

            surface_related: ManuallyDrop::new(surface_related),
        })
    }
}
