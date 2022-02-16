use anyhow::{Result, Error};
use ash::{vk::{Format, ColorSpaceKHR, Extent2D, SurfaceFormatKHR, PresentModeKHR, SwapchainKHR, SwapchainCreateInfoKHR, ImageUsageFlags, SharingMode, CompositeAlphaFlagsKHR}, extensions::khr::Swapchain, Instance, Device};
use winit::dpi::PhysicalSize;

use super::SurfaceRelated;
pub struct SwapchainRelated {
    pub surface_format: SurfaceFormatKHR,
    pub extent: Extent2D,
    pub present_mode: PresentModeKHR,
    pub loader: Swapchain,
    pub handle: SwapchainKHR,
    pub image_count: u32,
}

impl Drop for SwapchainRelated {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_swapchain(self.handle, None) }
    }
}

impl SwapchainRelated
{
    pub fn new(window_size: &PhysicalSize<u32>, instance: &Instance, device: &Device,surface_related: &SurfaceRelated) -> Result<Self> {
            let surface_format = *surface_related
                .formats
                .iter()
                .find(|f| {
                    f.format == Format::R8G8B8A8_UNORM
                        && f.color_space == ColorSpaceKHR::SRGB_NONLINEAR
                })
                .ok_or(Error::msg("No suitable format"))?;
            let extent = if surface_related.capabilities.current_extent.height == std::u32::MAX {
                // The extent of the swapchain can be choosen freely
                surface_related.capabilities.current_extent
            } else {
                Extent2D {
                    width: std::cmp::max(
                        surface_related.capabilities.min_image_extent.width,
                        std::cmp::min(
                            surface_related.capabilities.max_image_extent.width,
                            window_size.width,
                        ),
                    ),
                    height: std::cmp::max(
                        surface_related.capabilities.min_image_extent.height,
                        std::cmp::min(
                            surface_related.capabilities.max_image_extent.height,
                            window_size.height,
                        ),
                    ),
                }
            };
            // we don't want the window to block our rendering
            let present_mode = *surface_related
                .present_modes
                .iter()
                .find(|&&m| m == PresentModeKHR::IMMEDIATE)
                .ok_or(Error::msg("No suitable present mode"))?;
            let loader = Swapchain::new(instance, device);
            // let's try for at least 3 swapchain elements
            let image_count = if surface_related.capabilities.max_image_count > 0 {
                3u32.min(surface_related.capabilities.max_image_count)
            } else {
                3
            };
            let handle = unsafe {
                loader.create_swapchain(
                    &SwapchainCreateInfoKHR::builder()
                        .surface(surface_related.surface)
                        .min_image_count(image_count)
                        .image_color_space(surface_format.color_space)
                        .image_format(surface_format.format)
                        .image_extent(extent)
                        .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
                        .image_sharing_mode(SharingMode::EXCLUSIVE) // change this if present queue fam. differs
                        .pre_transform(surface_related.capabilities.current_transform)
                        .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
                        .present_mode(present_mode)
                        .clipped(true)
                        .image_array_layers(1),
                    None,
                )
            }?;

            Ok(Self {
                surface_format,
                extent,
                present_mode,
                loader,
                handle,
                image_count,
            })
        }
    }