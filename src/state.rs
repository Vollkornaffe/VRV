use std::mem::ManuallyDrop;

use anyhow::Result;
use ash::vk::{ImageAspectFlags, ImageTiling, ImageUsageFlags, MemoryPropertyFlags, RenderPass};
use winit::window::Window;

use crate::{wrap_openxr, wrap_vulkan};

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub swapchain_window: ManuallyDrop<wrap_vulkan::SwapchainRelated>,
    pub render_pass_window: RenderPass,
    pub depth_image_window: wrap_vulkan::DeviceImage,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.depth_image_window.drop(&self.vulkan.device);
            self.vulkan
                .device
                .destroy_render_pass(self.render_pass_window, None);
            self.swapchain_window.drop(&self.vulkan.device);
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
        let mut swapchain_window =
            wrap_vulkan::SwapchainRelated::new(&window.inner_size(), &vulkan)?;

        let depth_format = vulkan.find_supported_depth_stencil_format()?;

        let render_pass_window = wrap_vulkan::create_render_pass_window(
            &vulkan,
            swapchain_window.surface_format.format,
            depth_format,
        )?;
        let depth_image_window = wrap_vulkan::DeviceImage::new(
            &vulkan,
            wrap_vulkan::device_image::DeviceImageSettings {
                extent: swapchain_window.extent,
                format: depth_format,
                tiling: ImageTiling::OPTIMAL,
                usage: ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                properties: MemoryPropertyFlags::DEVICE_LOCAL,
                aspect_flags: ImageAspectFlags::DEPTH,
                name: "DepthWindow".to_string(),
            },
        )?;

        swapchain_window.fill_elements(&vulkan, depth_image_window.view, render_pass_window)?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            swapchain_window: ManuallyDrop::new(swapchain_window),
            render_pass_window,
            depth_image_window,
        })
    }
}
