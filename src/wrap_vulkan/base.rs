use anyhow::{bail, Error, Result};
use std::{
    ffi::{CStr, CString},
    mem::ManuallyDrop,
    ops::BitAnd,
};
use winit::window::Window;

use ash::{
    extensions::{ext::DebugUtils, khr::Swapchain},
    vk::{
        api_version_major, api_version_minor, make_api_version, ApplicationInfo,
        DebugUtilsObjectNameInfoEXT, DeviceCreateInfo, DeviceQueueCreateInfo, Format,
        FormatFeatureFlags, Handle, ImageTiling, InstanceCreateInfo, PhysicalDevice, QueueFlags,
    },
    Device, Entry, Instance,
};

use crate::wrap_openxr;

#[cfg(feature = "validation_vulkan")]
use super::Debug;
use super::SurfaceRelated;

pub struct Base {
    pub entry: ManuallyDrop<Entry>,
    pub instance: ManuallyDrop<Instance>,
    pub physical_device: ManuallyDrop<PhysicalDevice>,
    pub device: ManuallyDrop<Device>,

    #[cfg(feature = "validation_vulkan")]
    pub debug: ManuallyDrop<Debug>,

    pub queue_family_index: u32,
    pub surface_related: ManuallyDrop<SurfaceRelated>,
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.surface_related);
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self.physical_device);
            #[cfg(feature = "validation_vulkan")]
            ManuallyDrop::drop(&mut self.debug);
            ManuallyDrop::drop(&mut self.instance);
            ManuallyDrop::drop(&mut self.entry);
        }
    }
}

impl Base {
    pub fn new(window: &Window, wrap_openxr: &wrap_openxr::State) -> Result<Base> {
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

        let reqs = wrap_openxr.get_graphics_requirements()?;
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
            ash_window::enumerate_required_extensions(window)?
                .iter()
                .map(|&x| x.into())
                .collect(),
            wrap_openxr.get_instance_extensions()?,
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

        for (i, physical_device) in unsafe { instance.enumerate_physical_devices() }?
            .iter()
            .enumerate()
        {
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
            PhysicalDevice::from_raw(wrap_openxr.get_physical_device(instance.handle().as_raw())?);

        let physical_device_extension_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device) }?;
        for prop in &physical_device_extension_properties {
            log::trace!("{:?}", unsafe {
                CStr::from_ptr(prop.extension_name.as_ptr())
            });
        }

        let device_extensions: Vec<CString> = [
            wrap_openxr.get_device_extensions()?,
            vec![Swapchain::name().into()],
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

        let surface_related = SurfaceRelated::new(&entry, &instance, physical_device, window)?;

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
                        surface_related.loader.get_physical_device_surface_support(
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

        Ok(Self {
            entry: ManuallyDrop::new(entry),
            instance: ManuallyDrop::new(instance),
            physical_device: ManuallyDrop::new(physical_device),
            device: ManuallyDrop::new(device),

            #[cfg(feature = "validation_vulkan")]
            debug: ManuallyDrop::new(debug),

            queue_family_index,
            surface_related: ManuallyDrop::new(surface_related),
        })
    }

    #[cfg(feature = "validation_vulkan")]
    pub fn name_object<T: Clone + Handle>(&self, ash_object: &T, name: String) {
        let c_str = std::ffi::CString::new(name).unwrap();
        log::debug!(
            "Naming object {:?} of type {:?}: {:?}",
            ash_object.clone().as_raw(),
            T::TYPE,
            c_str
        );

        let name_info = DebugUtilsObjectNameInfoEXT::builder()
            .object_type(T::TYPE)
            .object_handle(ash_object.clone().as_raw())
            .object_name(&c_str);
        unsafe {
            self.debug
                .loader
                .debug_utils_set_object_name(self.device.handle(), &name_info)
        }
        .unwrap();
    }
    #[cfg(not(feature = "validation_vulkan"))]
    pub fn name_object<T: Clone + Handle>(&self, _: &T, _: String) {}

    pub fn find_supported_format(
        &self,
        candidates: Vec<Format>,
        tiling: ImageTiling,
        features: FormatFeatureFlags,
    ) -> Result<Format> {
        candidates
            .iter()
            .find_map(|&format| {
                let properties = unsafe {
                    self.instance
                        .get_physical_device_format_properties(*self.physical_device, format)
                };

                if match tiling {
                    ash::vk::ImageTiling::LINEAR => {
                        properties.linear_tiling_features.bitand(features) == features
                    }
                    ash::vk::ImageTiling::OPTIMAL => {
                        properties.optimal_tiling_features.bitand(features) == features
                    }
                    _ => false,
                } {
                    Some(format)
                } else {
                    None
                }
            })
            .ok_or(Error::msg("Couldn't find supported format"))
    }
}
