pub mod swapchain;
use std::mem::ManuallyDrop;

use anyhow::{Error, Result};
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBuffer, CommandBufferBeginInfo,
    CommandBufferResetFlags, DescriptorSet, Extent2D, Fence, IndexType, Offset2D, Pipeline,
    PipelineBindPoint, PipelineLayout, PipelineStageFlags, PresentInfoKHR, Rect2D, RenderPass,
    RenderPassBeginInfo, Semaphore, SubmitInfo, SubpassContents, Viewport,
};

use openxr::{
    CompositionLayerProjection, CompositionLayerProjectionView, Duration, EnvironmentBlendMode,
    Extent2Di, FrameState, FrameStream, FrameWaiter, Offset2Di, Posef, Rect2Di, ReferenceSpaceType,
    Session, Space, SwapchainSubImage, ViewConfigurationType, Vulkan,
};
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self, create_render_pass_window,
        geometry::MeshBuffers,
        render_pass::create_render_pass_hmd,
        sync::{create_fence, create_semaphore, wait_and_reset},
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

pub struct PreRenderInfoWindow {
    pub image_index: u32,
    image_acquired_semaphore: Semaphore,
}
pub struct PreRenderInfoHMD {
    pub image_index: Option<u32>, // is None if the app shouldn't render
    pub frame_state: FrameState,
}

impl State {
    pub fn pre_render_hmd(&mut self) -> Result<PreRenderInfoHMD> {
        let frame_state = self.frame_wait.wait()?;
        self.frame_stream.begin()?;

        let image_index = if frame_state.should_render {
            Some(self.hmd_swapchain.swapchain.acquire_image()?)
        } else {
            None
        };

        Ok(PreRenderInfoHMD {
            image_index,
            frame_state,
        })
    }

    pub fn pre_render_window(&mut self) -> Result<PreRenderInfoWindow> {
        // prepare semaphore
        let image_acquired_semaphore =
            self.window_semaphores_image_acquired[self.last_used_acquire_semaphore];
        self.last_used_acquire_semaphore += 1;
        self.last_used_acquire_semaphore %= self.window_semaphores_image_acquired.len();

        // acuire image
        let (image_index, _suboptimal) = unsafe {
            self.window_swapchain.loader.acquire_next_image(
                self.window_swapchain.handle,
                std::u64::MAX, // don't timeout
                image_acquired_semaphore,
                ash::vk::Fence::default(),
            )
        }?;

        Ok(PreRenderInfoWindow {
            image_index,
            image_acquired_semaphore,
        })
    }

    pub fn render_hmd(&mut self, pre_render_info: PreRenderInfoHMD) -> Result<()> {
        let PreRenderInfoHMD {
            image_index,
            frame_state,
        } = pre_render_info;

        // abort rendering
        if image_index.is_none() {
            self.frame_stream.end(
                frame_state.predicted_display_time,
                EnvironmentBlendMode::OPAQUE,
                &[],
            )?;
            return Ok(());
        }
        let image_index = image_index.unwrap();

        // Wait until the image is available to render to. The compositor could still be
        // reading from it.
        self.hmd_swapchain
            .swapchain
            .wait_image(Duration::INFINITE)?;

        // wait for rendering operations
        wait_and_reset(
            &self.vulkan,
            self.hmd_fences_rendering_finished[image_index as usize],
        )?;

        // TODO record drawing operations

        // Fetch the view transforms. To minimize latency, we intentionally do this *after*
        // recording commands to render the scene, i.e. at the last possible moment before
        // rendering begins in earnest on the GPU. Uniforms dependent on this data can be sent
        // to the GPU just-in-time by writing them to per-frame host-visible memory which the
        // GPU will only read once the command buffer is submitted.
        let (_, view_vec) = self.session.locate_views(
            ViewConfigurationType::PRIMARY_STEREO,
            frame_state.predicted_display_time,
            &self.stage,
        )?;
        let views = [view_vec[0], view_vec[1]];

        // TODO write camera matrices

        // TODO submit rendering commands

        self.hmd_swapchain.swapchain.release_image()?;

        self.frame_stream.end(
            frame_state.predicted_display_time,
            EnvironmentBlendMode::OPAQUE,
            &[&CompositionLayerProjection::new().space(&self.stage).views(
                &views
                    .iter()
                    .enumerate()
                    .map(|(i, view)| {
                        CompositionLayerProjectionView::new()
                            .pose(view.pose)
                            .fov(view.fov)
                            .sub_image(
                                SwapchainSubImage::new()
                                    .swapchain(&self.hmd_swapchain.swapchain)
                                    .image_array_index(i as u32)
                                    .image_rect(Rect2Di {
                                        offset: Offset2Di::default(),
                                        extent: Extent2Di {
                                            width: self.hmd_swapchain.extent.width as i32,
                                            height: self.hmd_swapchain.extent.height as i32,
                                        },
                                    }),
                            )
                    })
                    .collect::<Vec<_>>(),
            )],
        )?;

        Ok(())
    }

    pub fn render_window(
        &self,
        pre_render_info: PreRenderInfoWindow,
        pipeline_layout: PipelineLayout,
        pipeline: Pipeline,
        mesh: &MeshBuffers,
        descriptor_set: DescriptorSet,
    ) -> Result<()> {
        let PreRenderInfoWindow {
            image_index,
            image_acquired_semaphore,
        } = pre_render_info;

        // get the other stuff now that we know the index
        let rendering_finished_semaphore =
            self.window_semaphores_rendering_finished[image_index as usize];
        let rendering_finished_fence = self.window_fences_rendering_finished[image_index as usize];
        let command_buffer = self.window_command_buffers[image_index as usize];

        // waite before resetting cmd buffer
        wait_and_reset(&self.vulkan, rendering_finished_fence)?;

        // for convenience
        let window_extent = self.window_swapchain.extent;
        unsafe {
            let d = &self.vulkan.device;

            d.reset_command_buffer(command_buffer, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(command_buffer, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                command_buffer,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.window_render_pass)
                    .framebuffer(self.window_swapchain.elements[image_index as usize].frame_buffer)
                    .render_area(*Rect2D::builder().extent(window_extent))
                    .clear_values(&[
                        ClearValue {
                            color: ClearColorValue::default(),
                        },
                        ClearValue {
                            depth_stencil: ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            },
                        },
                    ]),
                SubpassContents::INLINE,
            );
            d.cmd_bind_pipeline(command_buffer, PipelineBindPoint::GRAPHICS, pipeline);

            // set this here so we don't have to recreate pipeline on window resize
            d.cmd_set_viewport(
                command_buffer,
                0,
                &[Viewport::builder()
                    .x(0.0)
                    .y(0.0)
                    .width(window_extent.width as f32)
                    .height(window_extent.height as f32)
                    .min_depth(0.0)
                    .max_depth(1.0)
                    .build()],
            );
            d.cmd_set_scissor(
                command_buffer,
                0,
                &[Rect2D::builder()
                    .offset(Offset2D { x: 0, y: 0 })
                    .extent(window_extent)
                    .build()],
            );

            d.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertex.handle()], &[0]);
            d.cmd_bind_index_buffer(command_buffer, mesh.index.handle(), 0, IndexType::UINT32);
            d.cmd_bind_descriptor_sets(
                command_buffer,
                PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );
            d.cmd_draw_indexed(command_buffer, mesh.num_indices() as u32, 1, 0, 0, 0);
            d.cmd_end_render_pass(command_buffer);
            d.end_command_buffer(command_buffer)?;

            self.vulkan.device.queue_submit(
                self.vulkan.queue,
                &[SubmitInfo::builder()
                    .command_buffers(&[command_buffer])
                    .wait_semaphores(&[image_acquired_semaphore])
                    .wait_dst_stage_mask(&[PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                    .signal_semaphores(&[rendering_finished_semaphore])
                    .build()],
                self.window_fences_rendering_finished[image_index as usize],
            )?;

            let _suboptimal = self.window_swapchain.loader.queue_present(
                self.vulkan.queue,
                &PresentInfoKHR::builder()
                    .wait_semaphores(&[rendering_finished_semaphore])
                    .swapchains(&[self.window_swapchain.handle])
                    .image_indices(&[image_index]),
            )?;
        }

        Ok(())
    }

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
}
