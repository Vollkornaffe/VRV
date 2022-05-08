pub mod render_hmd;
pub mod render_window;
pub mod swapchain;
pub mod texture;
use std::mem::ManuallyDrop;

use anyhow::{Error, Result};
use ash::vk::{CommandBuffer, Extent2D, Fence, RenderPass, Semaphore};

use openxr::{
    FrameState, FrameStream, FrameWaiter, Posef, ReferenceSpaceType, Session, Space, Time, View,
    ViewConfigurationType, Vulkan,
};
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self, create_render_pass_window,
        render_pass::create_render_pass_hmd,
        sync::{create_fence, create_semaphore},
    },
};
use swapchain::{SwapchainHMD, SwapchainWindow};

pub struct ContextHMD {
    frame_wait: FrameWaiter,
    frame_stream: FrameStream<Vulkan>,

    pub render_pass: RenderPass,
    pub swapchain: SwapchainHMD,

    // these are indexed by the result of acquiring
    pub command_buffers: Vec<CommandBuffer>,
    pub fences_rendering_finished: Vec<Fence>,
}

pub struct ContextWindow {
    // the acquiring semaphores are used round-robin
    // because we need to supply a semaphore prior to knowing which frame to use
    last_used_acquire_semaphore: usize,
    semaphores_image_acquired: Vec<Semaphore>,

    pub render_pass: RenderPass,
    pub swapchain: SwapchainWindow,

    // these are indexed by the result of acquiring
    pub command_buffers: Vec<CommandBuffer>,
    pub semaphores_rendering_finished: Vec<Semaphore>,
    pub fences_rendering_finished: Vec<Fence>,
}

pub struct Context {
    pub openxr: ManuallyDrop<wrap_openxr::Context>,
    pub vulkan: ManuallyDrop<wrap_vulkan::Context>,

    pub session: Session<Vulkan>,
    pub stage: Space,
    // TODO: actions
    pub hmd: ContextHMD,
    pub window: ContextWindow,
}

impl Drop for Context {
    fn drop(&mut self) {
        self.vulkan.wait_idle().unwrap();

        unsafe {
            self.window.swapchain.destroy(&self.vulkan);

            for &s in &self.window.semaphores_image_acquired {
                self.vulkan.device.destroy_semaphore(s, None);
            }
            for &s in &self.window.semaphores_rendering_finished {
                self.vulkan.device.destroy_semaphore(s, None);
            }
            for &f in &self.window.fences_rendering_finished {
                self.vulkan.device.destroy_fence(f, None);
            }

            self.vulkan
                .device
                .destroy_render_pass(self.window.render_pass, None);

            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
        }
    }
}

#[derive(Copy, Clone)]
pub struct PreRenderInfoWindow {
    pub image_index: u32,
    image_acquired_semaphore: Semaphore,
}
#[derive(Copy, Clone)]
pub struct PreRenderInfoHMD {
    pub image_index: Option<u32>,
    pub frame_state: FrameState,
}

impl Context {
    pub fn resize(&mut self, window: &Window) -> Result<()> {
        self.vulkan.wait_idle()?;

        unsafe { self.window.swapchain.destroy(&self.vulkan) };

        self.window.swapchain = SwapchainWindow::new(
            &self.vulkan,
            self.window.render_pass,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;
        Ok(())
    }

    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::Context::new()?;
        let vulkan = wrap_vulkan::Context::new(window, &openxr)?;

        // Setup HMD, from this point SteamVR needs to be available

        let (session, frame_wait, frame_stream) = openxr.init_with_vulkan(&vulkan)?;
        let stage = session.create_reference_space(ReferenceSpaceType::STAGE, Posef::IDENTITY)?;

        let hmd = {
            let render_pass = create_render_pass_hmd(&vulkan)?;
            let swapchain = SwapchainHMD::new(&openxr, &vulkan, render_pass, &session)?;
            let image_count = swapchain.elements.len() as u32;
            ContextHMD {
                frame_wait,
                frame_stream,
                render_pass,
                swapchain,
                command_buffers: vulkan
                    .alloc_command_buffers(image_count, "HMDCommandBuffers".to_string())?,
                fences_rendering_finished: (0..image_count)
                    .into_iter()
                    .map(|index| {
                        Ok(create_fence(
                            &vulkan,
                            true, // start in signaled state
                            format!("HMDFenceRenderingFinished_{}", index),
                        )?)
                    })
                    .collect::<Result<_, Error>>()?,
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
                )?,
                command_buffers: vulkan
                    .alloc_command_buffers(image_count, "WindowCommandBuffers".to_string())?,
                semaphores_rendering_finished: (0..image_count)
                    .into_iter()
                    .map(|index| {
                        Ok(create_semaphore(
                            &vulkan,
                            format!("WindowSemaphoreRenderingFinished_{}", index),
                        )?)
                    })
                    .collect::<Result<_, Error>>()?,
                fences_rendering_finished: (0..image_count)
                    .into_iter()
                    .map(|index| {
                        Ok(create_fence(
                            &vulkan,
                            true, // start in signaled state
                            format!("WindowFenceRenderingFinished_{}", index),
                        )?)
                    })
                    .collect::<Result<_, Error>>()?,
            }
        };

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),

            session,
            stage,

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
        let (_, view_vec) = self.session.locate_views(
            ViewConfigurationType::PRIMARY_STEREO,
            display_time,
            &self.stage,
        )?;
        Ok([view_vec[0], view_vec[1]])
    }
}