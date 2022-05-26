use anyhow::{bail, Error, Result};
use ash::{
    extensions::khr::Surface,
    vk::{
        ColorSpaceKHR, Format, PhysicalDevice, PresentModeKHR, SurfaceCapabilitiesKHR,
        SurfaceFormatKHR, SurfaceKHR,
    },
    Entry, Instance,
};
use winit::window::Window;

use super::Context;

pub struct SurfaceRelated {
    pub loader: Surface,
    pub surface: SurfaceKHR,
}

impl Drop for SurfaceRelated {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.surface, None) }
    }
}
pub struct Detail {
    pub capabilities: SurfaceCapabilitiesKHR,
    pub format: SurfaceFormatKHR,
    pub present_modes: Vec<PresentModeKHR>,
    pub image_count: u32,
}

impl SurfaceRelated {
    fn detail(
        loader: &Surface,
        physical_device: PhysicalDevice,
        surface: SurfaceKHR,
    ) -> Result<Detail> {
        let capabilities =
            unsafe { loader.get_physical_device_surface_capabilities(physical_device, surface) }?;
        let formats =
            unsafe { loader.get_physical_device_surface_formats(physical_device, surface) }?;
        let present_modes =
            unsafe { loader.get_physical_device_surface_present_modes(physical_device, surface) }?;
        if formats.is_empty() || present_modes.is_empty() {
            bail!("Physical device incompatible with surface")
        }
        let format = *formats
            .iter()
            .find(|f| {
                (f.format == Format::R8G8B8A8_UNORM || f.format == Format::B8G8R8A8_UNORM)
                    && f.color_space == ColorSpaceKHR::SRGB_NONLINEAR
            })
            .ok_or(Error::msg("No suitable surface format"))?;

        let image_count = if capabilities.max_image_count > 0 {
            3u32.min(capabilities.max_image_count)
        } else {
            3
        };

        Ok(Detail {
            capabilities,
            format,
            present_modes,
            image_count,
        })
    }

    pub fn new(entry: &Entry, instance: &Instance, window: &Window) -> Result<Self> {
        let loader = Surface::new(entry, instance);
        let surface = unsafe { ash_window::create_surface(entry, instance, &window, None) }?;

        Ok(Self { loader, surface })
    }

    pub fn get_detail(&self, context: &Context) -> Result<Detail> {
        Self::detail(&self.loader, context.physical_device, self.surface)
    }
}
