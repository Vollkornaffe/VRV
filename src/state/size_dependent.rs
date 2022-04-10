use anyhow::Result;
use ash::vk::{
    Extent2D, ImageAspectFlags, ImageTiling, ImageUsageFlags, MemoryPropertyFlags, RenderPass,
};

use crate::wrap_vulkan::{device_image::DeviceImageSettings, Base, DeviceImage, SwapchainRelated};

pub struct SizeDependentState {
    pub extent: Extent2D,
    pub depth_image: DeviceImage,
    pub swapchain: SwapchainRelated,
}

impl SizeDependentState {
    pub fn new(base: &Base, render_pass: RenderPass, wanted: Extent2D) -> Result<Self> {
        let depth_format = base.find_supported_depth_stencil_format()?;
        let extent = base.get_allowed_extend(wanted)?;

        let depth_image = DeviceImage::new(
            base,
            DeviceImageSettings {
                extent: extent,
                format: depth_format,
                tiling: ImageTiling::OPTIMAL,
                usage: ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                properties: MemoryPropertyFlags::DEVICE_LOCAL,
                aspect_flags: ImageAspectFlags::DEPTH,
                name: "DepthWindow".to_string(),
            },
        )?;

        let swapchain = SwapchainRelated::new(base, render_pass, extent, depth_image.view)?;

        Ok(Self {
            extent,
            depth_image,
            swapchain,
        })
    }

    pub unsafe fn destroy(&self, base: &Base) {
        self.swapchain.destroy(base);
        self.depth_image.destroy(&base);
    }
}
