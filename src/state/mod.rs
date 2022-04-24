pub mod render_hmd;
pub mod render_window;
pub mod swapchain;
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

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::Base>,
    pub vulkan: ManuallyDrop<wrap_vulkan::Base>,

    pub session: Session<Vulkan>,

    frame_wait: FrameWaiter,
    frame_stream: FrameStream<Vulkan>,

    stage: Space,

    pub hmd_render_pass: RenderPass,
    hmd_swapchain: SwapchainHMD,
    hmd_command_buffers: Vec<CommandBuffer>,
    hmd_fences_rendering_finished: Vec<Fence>,

    // TODO: actions

    // the acquiring semaphores are used round-robin
    // because we need to supply a semaphore prior to knowing which frame to use
    last_used_acquire_semaphore: usize,
    window_semaphores_image_acquired: Vec<Semaphore>,
    // these are indexed by the result of acquiring
    window_semaphores_rendering_finished: Vec<Semaphore>,
    window_fences_rendering_finished: Vec<Fence>,
    window_command_buffers: Vec<CommandBuffer>,

    pub window_render_pass: RenderPass,
    window_swapchain: SwapchainWindow,
}

impl Drop for State {
    fn drop(&mut self) {
        self.vulkan.wait_idle().unwrap();

        unsafe {
            self.window_swapchain.destroy(&self.vulkan);

            for &s in &self.window_semaphores_image_acquired {
                self.vulkan.device.destroy_semaphore(s, None);
            }
            for &s in &self.window_semaphores_rendering_finished {
                self.vulkan.device.destroy_semaphore(s, None);
            }
            for &f in &self.window_fences_rendering_finished {
                self.vulkan.device.destroy_fence(f, None);
            }

            self.vulkan
                .device
                .destroy_render_pass(self.window_render_pass, None);

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

impl State {
    pub fn resize(&mut self, window: &Window) -> Result<()> {
        self.vulkan.wait_idle()?;

        unsafe { self.window_swapchain.destroy(&self.vulkan) };

        self.window_swapchain = SwapchainWindow::new(
            &self.vulkan,
            self.window_render_pass,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;
        Ok(())
    }

    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::Base::new()?;
        let vulkan = wrap_vulkan::Base::new(window, &openxr)?;

        // Setup HMD, from this point SteamVR needs to be available

        let (session, frame_wait, frame_stream) = openxr.init_with_vulkan(&vulkan)?;
        let stage = session.create_reference_space(ReferenceSpaceType::STAGE, Posef::IDENTITY)?;

        let hmd_render_pass = create_render_pass_hmd(&vulkan)?;

        let hmd_swapchain = SwapchainHMD::new(&openxr, &vulkan, hmd_render_pass, &session)?;
        let hmd_image_count = hmd_swapchain.elements.len() as u32;
        let hmd_command_buffers =
            vulkan.alloc_command_buffers(hmd_image_count, "HMDCommandBuffers".to_string())?;
        let hmd_fences_rendering_finished = (0..hmd_image_count)
            .into_iter()
            .map(|index| {
                Ok(create_fence(
                    &vulkan,
                    true, // start in signaled state
                    format!("HMDFenceRenderingFinished_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        // Setup Window

        let window_render_pass = create_render_pass_window(&vulkan)?;

        let window_image_count = vulkan.get_image_count()?;

        let window_semaphores_image_acquired = (0..window_image_count)
            .into_iter()
            .map(|index| {
                Ok(create_semaphore(
                    &vulkan,
                    format!("WindowSemaphoreImageAcquired_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let window_semaphores_rendering_finished = (0..window_image_count)
            .into_iter()
            .map(|index| {
                Ok(create_semaphore(
                    &vulkan,
                    format!("WindowSemaphoreRenderingFinished_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let window_fences_rendering_finished = (0..window_image_count)
            .into_iter()
            .map(|index| {
                Ok(create_fence(
                    &vulkan,
                    true, // start in signaled state
                    format!("WindowFenceRenderingFinished_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let window_swapchain = SwapchainWindow::new(
            &vulkan,
            window_render_pass,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;

        let window_command_buffers =
            vulkan.alloc_command_buffers(window_image_count, "WindowCommandBuffers".to_string())?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),

            session,
            frame_wait,
            frame_stream,
            stage,

            hmd_render_pass,
            hmd_swapchain,
            hmd_command_buffers,
            hmd_fences_rendering_finished,

            last_used_acquire_semaphore: 0,
            window_semaphores_image_acquired,
            window_semaphores_rendering_finished,
            window_fences_rendering_finished,
            window_render_pass,
            window_command_buffers,
            window_swapchain,
        })
    }

    pub fn get_image_count_hmd(&self) -> u32 {
        self.hmd_swapchain.elements.len() as u32
    }

    pub fn get_image_count_window(&self) -> u32 {
        self.window_swapchain.elements.len() as u32
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
