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
        api_version_major, api_version_minor, make_api_version, ApplicationInfo, CommandBuffer,
        CommandBufferAllocateInfo, CommandBufferBeginInfo, CommandBufferLevel, CommandPool,
        CommandPoolCreateFlags, CommandPoolCreateInfo, DeviceCreateInfo, DeviceQueueCreateInfo,
        Extent2D, Format, FormatFeatureFlags, Handle, ImageTiling, InstanceCreateInfo,
        MemoryPropertyFlags, PhysicalDevice, PhysicalDeviceMultiviewFeatures, Queue, QueueFlags,
        SubmitInfo,
    },
    Device, Entry, Instance,
};

use crate::wrap_openxr;

#[cfg(feature = "validation_vulkan")]
use super::Debug;
use super::{surface::Detail, sync::create_fence, SurfaceRelated};

pub struct Context {
    pub entry: ManuallyDrop<Entry>,
    pub instance: ManuallyDrop<Instance>,
    pub physical_device: ManuallyDrop<PhysicalDevice>,
    pub device: ManuallyDrop<Device>,

    #[cfg(feature = "validation_vulkan")]
    pub debug: ManuallyDrop<Debug>,

    pub queue_family_index: u32,
    pub window_surface_related: ManuallyDrop<SurfaceRelated>,

    pub pool: CommandPool,
    pub queue: Queue,
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.window_surface_related);
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self.physical_device);
            #[cfg(feature = "validation_vulkan")]
            ManuallyDrop::drop(&mut self.debug);
            ManuallyDrop::drop(&mut self.instance);
            ManuallyDrop::drop(&mut self.entry);
        }
    }
}

impl Context {
    pub fn new(window: &Window, wrap_openxr: &wrap_openxr::Context) -> Result<Context> {
        #[cfg(feature = "validation_vulkan")]
        const VALIDATION_LAYER_NAME: &'static str = "VK_LAYER_KHRONOS_validation";
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_name = CString::new(VALIDATION_LAYER_NAME).unwrap();
        #[cfg(feature = "validation_vulkan")]
        let c_str_layer_names = [c_str_layer_name.as_ptr()];

        #[cfg(not(feature = "validation_vulkan"))]
        let c_str_layer_names = [];

        log::info!("Creating new Vulkan State");

        let vk_target_version = make_api_version(0, 1, 1, 0); // seems good enough for multiview

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

        let instance_extensions: Vec<CString> = [
            ash_window::enumerate_required_extensions(window)?
                .iter()
                .map(|&x| -> CString { unsafe { CStr::from_ptr(x) }.into() }) // new rust version
                .collect::<Vec<_>>(),
            // hehe sneaky
            #[cfg(feature = "validation_vulkan")]
            vec![DebugUtils::name().into()],
        ]
        .concat::<CString>();

        log::trace!("Vulkan instance extensions: {:?}", instance_extensions);

        let entry = unsafe { Entry::load() }?;

        #[cfg(feature = "validation_vulkan")]
        let mut debug_info = Debug::info();

        // I really couldn't find a better way to do this
        // the problem is that push_next can't take a "null object"
        let instance = unsafe {
            wrap_openxr.get_vulkan_instance(
                &entry,
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
        let physical_device = wrap_openxr.get_vulkan_physical_device(&instance)?;

        let physical_device_extension_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device) }?;
        for prop in &physical_device_extension_properties {
            log::trace!("{:?}", unsafe {
                CStr::from_ptr(prop.extension_name.as_ptr())
            });
        }

        let device_extensions: Vec<CString> = vec![Swapchain::name().into()];

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

        let surface_related = SurfaceRelated::new(&entry, &instance, window)?;

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
            wrap_openxr.get_vulkan_device(
                &entry,
                &instance,
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
                    })
                    .push_next(&mut PhysicalDeviceMultiviewFeatures::builder().multiview(true)),
            )
        }?;

        let pool = unsafe {
            device.create_command_pool(
                &CommandPoolCreateInfo::builder()
                    .flags(
                        CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                            | CommandPoolCreateFlags::TRANSIENT,
                    )
                    .queue_family_index(queue_family_index),
                None,
            )
        }?;

        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok(Self {
            entry: ManuallyDrop::new(entry),
            instance: ManuallyDrop::new(instance),
            physical_device: ManuallyDrop::new(physical_device),
            device: ManuallyDrop::new(device),

            #[cfg(feature = "validation_vulkan")]
            debug: ManuallyDrop::new(debug),

            queue_family_index,
            window_surface_related: ManuallyDrop::new(surface_related),

            pool,
            queue,
        })
    }

    #[cfg(feature = "validation_vulkan")]
    pub fn name_object<T: Copy + Handle>(&self, ash_object: T, name: String) -> Result<()> {
        use ash::vk::DebugUtilsObjectNameInfoEXT;

        let c_str = std::ffi::CString::new(name).unwrap();
        log::debug!(
            "Naming object {:?} of type {:?}: {:?}",
            ash_object.as_raw(),
            T::TYPE,
            c_str
        );

        let name_info = DebugUtilsObjectNameInfoEXT::builder()
            .object_type(T::TYPE)
            .object_handle(ash_object.as_raw())
            .object_name(&c_str);
        Ok(unsafe {
            self.debug
                .loader
                .debug_utils_set_object_name(self.device.handle(), &name_info)
        }?)
    }
    #[cfg(not(feature = "validation_vulkan"))]
    pub fn name_object<T: Copy + Handle>(&self, _: T, _: String) -> Result<()> {
        Ok(())
    }

    pub fn find_supported_format(
        &self,
        candidates: &[Format],
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
                    ImageTiling::LINEAR => {
                        properties.linear_tiling_features.bitand(features) == features
                    }
                    ImageTiling::OPTIMAL => {
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

    pub fn find_supported_color_format(&self) -> Result<Format> {
        self.find_supported_format(
            &[
                // not sure if this is a good idea tbh
                Format::B8G8R8A8_SRGB,
                Format::R8G8B8A8_SRGB,
            ],
            ImageTiling::OPTIMAL,
            FormatFeatureFlags::COLOR_ATTACHMENT,
        )
    }

    pub fn find_supported_depth_stencil_format(&self) -> Result<Format> {
        self.find_supported_format(
            &[
                Format::D32_SFLOAT,
                Format::D32_SFLOAT_S8_UINT,
                Format::D24_UNORM_S8_UINT,
            ],
            ImageTiling::OPTIMAL,
            FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    pub fn find_memory_type_index(
        &self,
        memory_type_bits: MemoryPropertyFlags,
        required_properties: MemoryPropertyFlags,
    ) -> Result<u32> {
        let memory_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(*self.physical_device)
        };
        (0..memory_properties.memory_type_count)
            .into_iter()
            .find(|&i| {
                memory_type_bits.bitand(MemoryPropertyFlags::from_raw(1 << i))
                    == MemoryPropertyFlags::from_raw(1 << i)
                    && memory_properties.memory_types[i as usize]
                        .property_flags
                        .bitand(required_properties)
                        == required_properties
            })
            .ok_or(Error::msg("Failed to find suitable memory type"))
    }

    pub fn get_allowed_extend(&self, wanted: Extent2D) -> Result<Extent2D> {
        let Detail { capabilities, .. } = self.window_surface_related.get_detail(&self)?;
        Ok(if capabilities.current_extent.height == std::u32::MAX {
            Extent2D {
                width: std::cmp::max(
                    capabilities.min_image_extent.width,
                    std::cmp::min(capabilities.max_image_extent.width, wanted.width),
                ),
                height: std::cmp::max(
                    capabilities.min_image_extent.height,
                    std::cmp::min(capabilities.max_image_extent.height, wanted.height),
                ),
            }
        } else {
            // The extent of the swapchain can't be choosen freely, wanted is ignored
            capabilities.current_extent
        })
    }

    pub fn get_surface_format(&self) -> Result<Format> {
        Ok(self.window_surface_related.get_detail(&self)?.format.format)
    }

    pub fn get_image_count(&self) -> Result<u32> {
        Ok(self.window_surface_related.get_detail(&self)?.image_count)
    }

    pub fn wait_idle(&self) -> Result<()> {
        Ok(unsafe { self.device.queue_wait_idle(self.queue) }?)
    }

    pub fn alloc_command_buffers(&self, count: u32, name: String) -> Result<Vec<CommandBuffer>> {
        let buffers = unsafe {
            self.device.allocate_command_buffers(
                &CommandBufferAllocateInfo::builder()
                    .command_pool(self.pool)
                    .level(CommandBufferLevel::PRIMARY)
                    .command_buffer_count(count),
            )
        }?;

        for (i, &cb) in buffers.iter().enumerate() {
            self.name_object(cb, format!("{}_{}", name, i))?;
        }

        Ok(buffers)
    }

    pub fn one_shot<T, F: FnOnce(CommandBuffer) -> Result<T>>(
        &self,
        cmd_writer: F,
        name: String,
    ) -> Result<T> {
        let cmd = self.alloc_command_buffers(1, format!("{}CommandBuffer", name))?[0];
        let fence = create_fence(&self, false, format!("{}Fence", name))?;

        let cmd_result;
        unsafe {
            self.device
                .begin_command_buffer(cmd, &CommandBufferBeginInfo::builder())?;

            cmd_result = cmd_writer(cmd)?;

            self.device.end_command_buffer(cmd)?;
            self.device.queue_submit(
                self.queue,
                &[SubmitInfo::builder().command_buffers(&[cmd]).build()],
                fence,
            )?;
            self.device.wait_for_fences(
                &[fence],
                true,          // wait all
                std::u64::MAX, // don't timeout
            )?;
            self.device.free_command_buffers(self.pool, &[cmd]);
            self.device.destroy_fence(fence, None);
        }

        Ok(cmd_result)
    }
}
