use std::mem::ManuallyDrop;

use anyhow::Result;

use crate::{wrap_openxr, wrap_vulkan, wrap_window};

pub struct State {
    pub window: ManuallyDrop<wrap_window::State>,
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub swapchain_window: ManuallyDrop<wrap_vulkan::SwapchainRelated>,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.swapchain_window);
            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
            ManuallyDrop::drop(&mut self.window);
        }
    }
}

impl State {
    pub fn new() -> Result<Self> {
        log::info!("Creating new VRV state");

        let window = wrap_window::State::new();
        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::Base::new(&window, &openxr)?;
        let swapchain_window =
            wrap_vulkan::SwapchainRelated::new(&window.window.inner_size(), &vulkan)?;

        Ok(Self {
            window: ManuallyDrop::new(window),
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            swapchain_window: ManuallyDrop::new(swapchain_window),
        })
    }
}
