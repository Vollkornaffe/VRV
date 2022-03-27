use anyhow::{bail, Error, Result};
use ash::{
    extensions::khr::Swapchain,
    vk::{
        CompositeAlphaFlagsKHR, Extent2D, Framebuffer,
        FramebufferCreateInfo, Image, ImageAspectFlags, ImageUsageFlags, ImageView, PresentModeKHR,
        RenderPass, SharingMode, SwapchainCreateInfoKHR, SwapchainKHR,
    },
};


use super::{Base, DeviceImage, surface::Detail};

pub struct SwapElement {
    pub image: Image,
    pub view: ImageView,
    pub frame_buffer: Framebuffer,
}
pub struct SwapchainRelated {
    pub extent: Extent2D,
    pub present_mode: PresentModeKHR,
    pub loader: Swapchain,
    pub handle: SwapchainKHR,
    pub elements: Vec<SwapElement>,
}

impl SwapchainRelated {
    pub fn new(
        base: &Base,
        render_pass: RenderPass,
        extent: Extent2D,
        depth_view: ImageView,
    ) -> Result<Self> {
        let Detail { capabilities, present_modes, image_count, format } = base.surface_related.get_detail(base)?;

        // we don't want the window to block our rendering
        let present_mode = *present_modes
            .iter()
            .find(|&&m| m == PresentModeKHR::IMMEDIATE)
            .ok_or(Error::msg("No suitable present mode"))?;
        let loader = Swapchain::new(&base.instance, &base.device);
        let handle = unsafe {
            loader.create_swapchain(
                &SwapchainCreateInfoKHR::builder()
                    .surface(base.surface_related.surface)
                    .min_image_count(image_count)
                    .image_color_space(format.color_space)
                    .image_format(format.format)
                    .image_extent(extent)
                    .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(SharingMode::EXCLUSIVE) // change this if present queue fam. differs
                    .pre_transform(capabilities.current_transform)
                    .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(present_mode)
                    .clipped(true)
                    .image_array_layers(1),
                None,
            )
        }?;
        // there is also the HMD swapchain
        base.name_object(handle, "WindowSwapchain".to_string())?;

        let images = unsafe { loader.get_swapchain_images(handle) }?;
        for (i, &image) in images.iter().enumerate() {
            base.name_object(image, format!("WindowSwapchainImage_{}", i))?;
        }

        if images.len() != image_count as usize {
            bail!("Somehow the number of images in the swapchain doesn't add up");
        }

        let elements = (0..images.len())
            .into_iter()
            .map(|i| -> Result<SwapElement> {
                let image = images[i as usize];
                let view = DeviceImage::new_view(
                    base,
                    image,
                    format.format,
                    ImageAspectFlags::COLOR,
                    format!("WindowSwapchainView_{}", i),
                )?;

                let frame_buffer = unsafe {
                    base.device.create_framebuffer(
                        &FramebufferCreateInfo::builder()
                            .render_pass(render_pass)
                            .attachments(&[view, depth_view])
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1),
                        None,
                    )?
                };
                base.name_object(frame_buffer, format!("WindowSwapchainFrameBuffer_{}", i))?;

                Ok(SwapElement {
                    image,
                    view,
                    frame_buffer,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            extent,
            present_mode,
            loader,
            handle,
            elements,
        })
    }

    pub unsafe fn destroy(&self, base: &Base) {
        for e in &self.elements {
            base.device.destroy_image_view(e.view, None);
            base.device.destroy_framebuffer(e.frame_buffer, None);
        }
        self.loader.destroy_swapchain(self.handle, None)
    }
}
