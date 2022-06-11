pub mod render_hmd;
pub mod render_window;
pub mod swapchain;

use anyhow::{Error, Result};
use ash::{
    vk::{Extent2D, RenderPass, Semaphore, SwapchainKHR},
    Device,
};

use openxr::{
    FrameState, FrameStream, FrameWaiter, Posef, ReferenceSpaceType, Session, Space, Time, View,
    ViewConfigurationType, Vulkan,
};
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self, create_render_pass_window, render_pass::create_render_pass_hmd,
        sync::create_semaphore,
    },
};
use swapchain::{SwapchainHMD, SwapchainWindow};

pub struct ContextHMD {
    pub session: Session<Vulkan>,
    frame_wait: FrameWaiter,
    frame_stream: FrameStream<Vulkan>,
    pub stage: Space,

    pub render_pass: RenderPass,
    pub swapchain: SwapchainHMD,

    device: Device,
}

impl Drop for ContextHMD {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
            // rest implements drop
        }
    }
}

pub struct ContextWindow {
    // the acquiring semaphores are used round-robin
    // because we need to supply a semaphore prior to knowing which frame to use
    last_used_acquire_semaphore: usize,
    semaphores_image_acquired: Vec<Semaphore>,

    pub render_pass: RenderPass,
    pub swapchain: SwapchainWindow,

    device: Device,
}

impl Drop for ContextWindow {
    fn drop(&mut self) {
        unsafe {
            for semaphore in &self.semaphores_image_acquired {
                self.device.destroy_semaphore(*semaphore, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

pub struct Context {
    pub hmd: ContextHMD,
    pub window: ContextWindow,

    pub openxr: wrap_openxr::Context,
    pub vulkan: wrap_vulkan::Context,
}

#[derive(Copy, Clone)]
pub struct PreRenderInfoWindow {
    pub image_index: u32,
    pub image_acquired_semaphore: Semaphore,
}
#[derive(Copy, Clone)]
pub struct PreRenderInfoHMD {
    pub image_index: Option<u32>,
    pub frame_state: FrameState,
}

impl Context {
    pub fn resize(&mut self, window: &Window) -> Result<()> {
        self.vulkan.wait_idle()?;

        self.window.swapchain = SwapchainWindow::new(
            &self.vulkan,
            self.window.render_pass,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
            self.window.swapchain.handle,
        )?;
        Ok(())
    }

    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::Context::new()?;
        let vulkan = wrap_vulkan::Context::new(window, &openxr)?;

        // Setup HMD, from this point SteamVR needs to be available
        let hmd = {
            let (session, frame_wait, frame_stream) = openxr.init_with_vulkan(&vulkan)?;
            let stage =
                session.create_reference_space(ReferenceSpaceType::STAGE, Posef::IDENTITY)?;

            let render_pass = create_render_pass_hmd(&vulkan)?;
            let swapchain = SwapchainHMD::new(&openxr, &vulkan, render_pass, &session)?;
            ContextHMD {
                frame_wait,
                frame_stream,
                render_pass,
                swapchain,
                session,
                stage,
                device: vulkan.device.clone(),
            }
        };

        let window = {
            let image_count = vulkan.get_image_count()?;
            let render_pass = create_render_pass_window(&vulkan)?;
            ContextWindow {
                last_used_acquire_semaphore: 0,
                semaphores_image_acquired: (0..image_count)
                    .into_iter()
                    .map(|index| {
                        Ok(create_semaphore(
                            &vulkan,
                            format!("WindowSemaphoreImageAcquired_{}", index),
                        )?)
                    })
                    .collect::<Result<_, Error>>()?,
                render_pass,
                swapchain: SwapchainWindow::new(
                    &vulkan,
                    render_pass,
                    Extent2D {
                        width: window.inner_size().width,
                        height: window.inner_size().height,
                    },
                    SwapchainKHR::default(),
                )?,
                device: vulkan.device.clone(),
            }
        };

        Ok(Self {
            openxr,
            vulkan,

            hmd,
            window,
        })
    }

    pub fn get_image_count_hmd(&self) -> u32 {
        self.hmd.swapchain.elements.len() as u32
    }

    pub fn get_image_count_window(&self) -> u32 {
        self.window.swapchain.elements.len() as u32
    }

    pub fn get_views(&self, display_time: Time) -> Result<[View; 2]> {
        let (_, view_vec) = self.hmd.session.locate_views(
            ViewConfigurationType::PRIMARY_STEREO,
            display_time,
            &self.hmd.stage,
        )?;
        Ok([view_vec[0], view_vec[1]])
    }
}
