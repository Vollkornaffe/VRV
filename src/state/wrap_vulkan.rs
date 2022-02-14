use anyhow::{bail, Result};
use ash::{
    extensions::ext::DebugUtils,
    vk::{make_version, version_major, version_minor},
    Instance,
};

use super::wrap_openxr;

pub struct State {}

impl State {
    pub fn new(xr_base: &wrap_openxr::State) -> Result<State> {
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

        log::info!("Vulkan instance extensions: {:?}", instance_extensions);

        Ok(Self {})
    }
}
