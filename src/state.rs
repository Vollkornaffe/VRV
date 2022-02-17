use std::mem::ManuallyDrop;

use anyhow::Result;
use ash::vk::{
    ImageAspectFlags, ImageTiling, ImageUsageFlags, MemoryPropertyFlags, Pipeline, PipelineLayout,
    RenderPass,
};
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{self, command::CommandRelated, create_pipeline, create_pipeline_layout},
};

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub swapchain_window: ManuallyDrop<wrap_vulkan::SwapchainRelated>,
    pub render_pass_window: RenderPass,
    pub depth_image_window: wrap_vulkan::DeviceImage,

    pub pipeline_layout: PipelineLayout,
    pub pipeline: Pipeline,

    pub command_related: CommandRelated,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.vulkan
                .device
                .destroy_command_pool(self.command_related.pool, None);
            self.vulkan
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.vulkan.device.destroy_pipeline(self.pipeline, None);
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
    pub fn render(&self) -> Result<()> {
        unsafe {
            self.vulkan
                .device
                .queue_wait_idle(self.command_related.queue)
        }?;

        Ok(())
    }

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

        let pipeline_layout = create_pipeline_layout(&vulkan)?;
        let pipeline = create_pipeline(
            &vulkan,
            swapchain_window.extent,
            render_pass_window,
            pipeline_layout,
        )?;

        let command_related = CommandRelated::new(&vulkan)?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            swapchain_window: ManuallyDrop::new(swapchain_window),
            render_pass_window,
            depth_image_window,
            pipeline_layout,
            pipeline,
            command_related,
        })
    }
}
