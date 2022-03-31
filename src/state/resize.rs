use anyhow::Result;
use ash::vk::{
    Extent2D, ImageAspectFlags, ImageTiling, ImageUsageFlags, MemoryPropertyFlags,
    Pipeline, PipelineLayout, RenderPass,
};

use crate::wrap_vulkan::{
    create_pipeline, device_image::DeviceImageSettings, Base, DeviceImage, SwapchainRelated,
};

pub struct ResizableWindowState {
    pub extent: Extent2D,
    pub depth_image: DeviceImage,
    pub swapchain: SwapchainRelated,
    pub pipeline: Pipeline,
}

impl ResizableWindowState {
    pub fn new(
        base: &Base,
        render_pass: RenderPass,
        pipeline_layout: PipelineLayout,
        wanted: Extent2D,
    ) -> Result<Self> {
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

        let swapchain =
            SwapchainRelated::new(base, render_pass, extent, depth_image.view)?;

        let pipeline = create_pipeline(base, extent, render_pass, pipeline_layout)?;

        Ok(Self {
            extent,
            depth_image,
            swapchain,
            pipeline,
        })
    }

    pub unsafe fn destroy(&self, base: &Base) {
        base.device.destroy_pipeline(self.pipeline, None);
        self.swapchain.destroy(base);
        self.depth_image.destroy(&base);
    }
}
