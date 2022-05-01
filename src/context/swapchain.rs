use anyhow::{bail, Error, Result};
use ash::{
    extensions::khr::Swapchain,
    vk::{
        CompositeAlphaFlagsKHR, Extent2D, Framebuffer, FramebufferCreateInfo, Handle, Image,
        ImageAspectFlags, ImageTiling, ImageUsageFlags, ImageView, MemoryPropertyFlags,
        PresentModeKHR, RenderPass, SharingMode, SwapchainCreateInfoKHR, SwapchainKHR,
    },
};

use openxr::{Session, Vulkan};

use crate::{
    wrap_openxr,
    wrap_vulkan::{self, device_image::DeviceImageSettings, surface::Detail, DeviceImage},
};

pub struct SwapElement {
    pub image: Image,
    pub view: ImageView,
    pub frame_buffer: Framebuffer,
}
pub struct SwapchainWindow {
    pub extent: Extent2D,
    pub depth_image: DeviceImage,
    pub loader: Swapchain,
    pub handle: SwapchainKHR,
    pub elements: Vec<SwapElement>,
}

pub struct SwapchainHMD {
    pub extent: Extent2D,
    pub swapchain: openxr::Swapchain<Vulkan>,
    pub depth_image: DeviceImage,
    pub elements: Vec<SwapElement>,
}

impl SwapchainWindow {
    pub fn new(
        context: &wrap_vulkan::Context,
        render_pass: RenderPass,
        wanted: Extent2D,
    ) -> Result<Self> {
        let depth_format = context.find_supported_depth_stencil_format()?;
        let extent = context.get_allowed_extend(wanted)?;

        let depth_image = DeviceImage::new(
            context,
            DeviceImageSettings {
                extent: extent,
                format: depth_format,
                tiling: ImageTiling::OPTIMAL,
                usage: ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                properties: MemoryPropertyFlags::DEVICE_LOCAL,
                aspect_mask: ImageAspectFlags::DEPTH,
                layer_count: 1,
                name: "WindowDepth".to_string(),
            },
        )?;

        let Detail {
            capabilities,
            present_modes,
            image_count,
            format,
        } = context.window_surface_related.get_detail(context)?;

        // we don't want the window to block our rendering
        let present_mode = *present_modes
            .iter()
            .find(|&&m| m == PresentModeKHR::IMMEDIATE)
            .ok_or(Error::msg("No suitable present mode"))?;
        let loader = Swapchain::new(&context.instance, &context.device);
        let handle = unsafe {
            loader.create_swapchain(
                &SwapchainCreateInfoKHR::builder()
                    .surface(context.window_surface_related.surface)
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
        context.name_object(handle, "WindowSwapchain".to_string())?;

        let images = unsafe { loader.get_swapchain_images(handle) }?;
        for (i, &image) in images.iter().enumerate() {
            context.name_object(image, format!("WindowSwapchainImage_{}", i))?;
        }

        if images.len() != image_count as usize {
            bail!("Somehow the number of images in the swapchain doesn't add up");
        }

        let elements = (0..images.len())
            .into_iter()
            .map(|i| -> Result<SwapElement> {
                let image = images[i as usize];
                let view = DeviceImage::new_view(
                    context,
                    image,
                    format.format,
                    ImageAspectFlags::COLOR,
                    1,
                    format!("WindowSwapchainView_{}", i),
                )?;

                let frame_buffer = unsafe {
                    context.device.create_framebuffer(
                        &FramebufferCreateInfo::builder()
                            .render_pass(render_pass)
                            .attachments(&[view, depth_image.view])
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1),
                        None,
                    )?
                };
                context.name_object(frame_buffer, format!("WindowSwapchainFrameBuffer_{}", i))?;

                Ok(SwapElement {
                    image,
                    view,
                    frame_buffer,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            extent,
            depth_image,
            loader,
            handle,
            elements,
        })
    }

    pub unsafe fn destroy(&self, context: &wrap_vulkan::Context) {
        for e in &self.elements {
            context.device.destroy_image_view(e.view, None);
            context.device.destroy_framebuffer(e.frame_buffer, None);
        }
        self.loader.destroy_swapchain(self.handle, None);
        self.depth_image.destroy(context);
    }
}

impl SwapchainHMD {
    pub fn new(
        xr_context: &wrap_openxr::Context,
        vk_context: &wrap_vulkan::Context,
        render_pass: RenderPass,
        session: &Session<Vulkan>,
    ) -> Result<Self> {
        let extent = xr_context.get_resolution()?;

        let format = vk_context.find_supported_color_format()?;

        let swapchain = wrap_openxr::Context::get_swapchain(session, extent, format)?;

        let depth_image = DeviceImage::new(
            vk_context,
            DeviceImageSettings {
                extent: extent,
                format: vk_context.find_supported_depth_stencil_format()?,
                tiling: ImageTiling::OPTIMAL,
                usage: ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                properties: MemoryPropertyFlags::DEVICE_LOCAL,
                aspect_mask: ImageAspectFlags::DEPTH,
                layer_count: 2,
                name: "HMDDepth".to_string(),
            },
        )?;

        let elements = swapchain
            .enumerate_images()?
            .into_iter()
            .enumerate()
            .map(|(i, xr_image_handle)| -> Result<SwapElement> {
                let image = Image::from_raw(xr_image_handle);
                vk_context.name_object(image, format!("HMDSwapchainImage_{}", i))?;

                let view = DeviceImage::new_view(
                    vk_context,
                    image,
                    format,
                    ImageAspectFlags::COLOR,
                    2,
                    format!("HMDSwapchainView_{}", i),
                )?;

                let frame_buffer = unsafe {
                    vk_context.device.create_framebuffer(
                        &FramebufferCreateInfo::builder()
                            .render_pass(render_pass)
                            .attachments(&[view, depth_image.view])
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1), // multiview dictates this
                        None,
                    )
                }?;
                vk_context.name_object(frame_buffer, format!("HMDSwapchainFrameBuffer_{}", i))?;

                Ok(SwapElement {
                    image,
                    view,
                    frame_buffer,
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            extent,
            swapchain,
            depth_image,
            elements,
        })
    }
}
