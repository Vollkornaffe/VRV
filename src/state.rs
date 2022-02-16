use std::mem::ManuallyDrop;

use anyhow::Result;
use ash::vk::RenderPass;
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{self, create_window_render_pass},
};

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub swapchain_window: ManuallyDrop<wrap_vulkan::SwapchainRelated>,
    pub render_pass_window: RenderPass,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.vulkan
                .device
                .destroy_render_pass(self.render_pass_window, None);
            ManuallyDrop::drop(&mut self.swapchain_window);
            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
        }
    }
}

impl State {
    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::Base::new(window, &openxr)?;
        let swapchain_window = wrap_vulkan::SwapchainRelated::new(&window.inner_size(), &vulkan)?;
        let render_pass_window =
            create_window_render_pass(&vulkan, swapchain_window.surface_format.format)?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            swapchain_window: ManuallyDrop::new(swapchain_window),
            render_pass_window,
        })
    }
}
