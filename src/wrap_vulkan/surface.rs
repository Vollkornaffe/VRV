use anyhow::{bail, Result};
use ash::{
    extensions::khr::Surface,
    vk::{PhysicalDevice, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR},
    Entry, Instance,
};
use winit::window::Window;

pub struct SurfaceRelated {
    pub loader: Surface,
    pub surface: SurfaceKHR,
    pub capabilities: SurfaceCapabilitiesKHR,
    pub formats: Vec<SurfaceFormatKHR>,
    pub present_modes: Vec<PresentModeKHR>,
}

impl Drop for SurfaceRelated {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.surface, None) }
    }
}
impl SurfaceRelated {
    pub fn new(
        entry: &Entry,
        instance: &Instance,
        physical_device: PhysicalDevice,
        window: &Window,
    ) -> Result<Self> {
        let loader = Surface::new(entry, instance);
        let surface = unsafe { ash_window::create_surface(entry, instance, &window, None) }?;
        let capabilities =
            unsafe { loader.get_physical_device_surface_capabilities(physical_device, surface) }?;
        let formats =
            unsafe { loader.get_physical_device_surface_formats(physical_device, surface) }?;
        let present_modes =
            unsafe { loader.get_physical_device_surface_present_modes(physical_device, surface) }?;
        if formats.is_empty() || present_modes.is_empty() {
            bail!("Physical device incompatible with surface")
        }
        Ok(Self {
            loader,
            surface,
            capabilities,
            formats,
            present_modes,
        })
    }
}
