use anyhow::{bail, Error, Result};
use ash::{
    extensions::khr::Swapchain,
    vk::{
        ColorSpaceKHR, CompositeAlphaFlagsKHR, Extent2D, Format, Framebuffer,
        FramebufferCreateInfo, Image, ImageAspectFlags, ImageUsageFlags, ImageView, PresentModeKHR,
        RenderPass, SharingMode, SurfaceFormatKHR, SwapchainCreateInfoKHR, SwapchainKHR,
    },
    Device,
};
use winit::dpi::PhysicalSize;

use super::{Base, DeviceImage};

pub struct SwapElement {
    pub image: Image,
    pub view: ImageView,
    pub frame_buffer: Framebuffer,
}
pub struct SwapchainRelated {
    pub surface_format: SurfaceFormatKHR,
    pub extent: Extent2D,
    pub present_mode: PresentModeKHR,
    pub loader: Swapchain,
    pub handle: SwapchainKHR,
    pub image_count: u32,
    pub elements: Vec<SwapElement>,
}

impl SwapchainRelated {
    pub fn new(window_size: &PhysicalSize<u32>, base: &Base) -> Result<Self> {
        let surface_format = *base
            .surface_related
            .formats
            .iter()
            .find(|f| {
                (f.format == Format::R8G8B8A8_UNORM || f.format == Format::B8G8R8A8_UNORM)
                    && f.color_space == ColorSpaceKHR::SRGB_NONLINEAR
            })
            .ok_or(Error::msg("No suitable format"))?;
        let extent = if base.surface_related.capabilities.current_extent.height == std::u32::MAX {
            // The extent of the swapchain can be choosen freely
            base.surface_related.capabilities.current_extent
        } else {
            Extent2D {
                width: std::cmp::max(
                    base.surface_related.capabilities.min_image_extent.width,
                    std::cmp::min(
                        base.surface_related.capabilities.max_image_extent.width,
                        window_size.width,
                    ),
                ),
                height: std::cmp::max(
                    base.surface_related.capabilities.min_image_extent.height,
                    std::cmp::min(
                        base.surface_related.capabilities.max_image_extent.height,
                        window_size.height,
                    ),
                ),
            }
        };
        // we don't want the window to block our rendering
        let present_mode = *base
            .surface_related
            .present_modes
            .iter()
            .find(|&&m| m == PresentModeKHR::IMMEDIATE)
            .ok_or(Error::msg("No suitable present mode"))?;
        let loader = Swapchain::new(&base.instance, &base.device);
        // let's try for at least 3 swapchain elements
        let image_count = if base.surface_related.capabilities.max_image_count > 0 {
            3u32.min(base.surface_related.capabilities.max_image_count)
        } else {
            3
        };
        let handle = unsafe {
            loader.create_swapchain(
                &SwapchainCreateInfoKHR::builder()
                    .surface(base.surface_related.surface)
                    .min_image_count(image_count)
                    .image_color_space(surface_format.color_space)
                    .image_format(surface_format.format)
                    .image_extent(extent)
                    .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(SharingMode::EXCLUSIVE) // change this if present queue fam. differs
                    .pre_transform(base.surface_related.capabilities.current_transform)
                    .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(present_mode)
                    .clipped(true)
                    .image_array_layers(1),
                None,
            )
        }?;
        // there is also the HMD swapchain
        base.name_object(handle, "WindowSwapchain".to_string())?;

        Ok(Self {
            surface_format,
            extent,
            present_mode,
            loader,
            handle,
            image_count,
            elements: Vec::new(), // this is filled after the render pass is created, since the framebuffers reference it
        })
    }

    pub fn fill_elements(
        &mut self,
        base: &Base,
        depth_view: ImageView,
        render_pass: RenderPass,
    ) -> Result<()> {
        let images = unsafe { self.loader.get_swapchain_images(self.handle) }?;
        for (i, &image) in images.iter().enumerate() {
            base.name_object(image, format!("WindowSwapchainImage_{}", i))?;
        }

        if images.len() != self.image_count as usize {
            bail!("Somehow the number of images in the swapchain doesn't add up");
        }

        self.elements = (0..self.image_count)
            .into_iter()
            .map(|i| -> Result<SwapElement> {
                let image = images[i as usize];
                let view = DeviceImage::new_view(
                    base,
                    image,
                    self.surface_format.format,
                    ImageAspectFlags::COLOR,
                    format!("WindowSwapchainView_{}", i),
                )?;

                let frame_buffer = unsafe {
                    base.device.create_framebuffer(
                        &FramebufferCreateInfo::builder()
                            .render_pass(render_pass)
                            .attachments(&[view, depth_view])
                            .width(self.extent.width)
                            .height(self.extent.height)
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
        Ok(())
    }

    pub unsafe fn drop(&self, device: &Device) {
        for e in &self.elements {
            device.destroy_image_view(e.view, None);
            device.destroy_framebuffer(e.frame_buffer, None);
        }
        self.loader.destroy_swapchain(self.handle, None)
    }
}
