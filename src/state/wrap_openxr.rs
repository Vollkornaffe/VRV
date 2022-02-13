use anyhow::{bail, Result};
use openxr::{
    raw::VulkanEnableKHR, sys::Bool32, ApplicationInfo, Entry, ExtensionSet, Instance, SystemId,
};

pub struct State {
    pub entry: Entry,
    pub instance: Instance,
    //pub system_id: SystemId,
    //pub vk_fns: VulkanEnableKHR,
}

// TODO
// unsafe extern "system" fn openxr_debug_utils_callback() -> Bool32 {}

impl State {
    pub fn new() -> Result<State> {
        const validation_layer_name: &'static str = "XR_APILAYER_LUNARG_core_validation";

        log::info!("Creating new OpenXR State");

        let entry = Entry::linked();
        let available_extensions = entry.enumerate_extensions()?;
        let available_layers = entry.enumerate_layers()?;

        log::trace!("available_extensions: {:?}", available_extensions);
        log::trace!("available_layers: {:?}", available_layers);

        assert!(available_extensions.khr_vulkan_enable);

        if cfg!(feature = "validation_openxr") {
            assert!(
                available_layers
                    .iter()
                    .find(|l| l.layer_name == validation_layer_name)
                    .is_some(),
                "Validation layer not found, did you set XR_API_LAYER_PATH?"
            );
        } else {
            log::warn!("No OpenXR validation, enable with ");
        }

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
                &[validation_layer_name]
            } else {
                &[]
            },
        )?;

        Ok(State {
            entry,
            instance,
            //system_id,
            //vk_fns,
        })
    }
}
