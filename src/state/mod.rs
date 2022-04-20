mod size_dependent;

use std::mem::ManuallyDrop;

use anyhow::{Error, Result};
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBuffer, CommandBufferBeginInfo,
    CommandBufferResetFlags, DescriptorSet, Extent2D, Fence, IndexType, Offset2D, Pipeline,
    PipelineBindPoint, PipelineLayout, PipelineStageFlags, PresentInfoKHR, Rect2D, RenderPass,
    RenderPassBeginInfo, Semaphore, SubmitInfo, SubpassContents, Viewport,
};

use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self, create_render_pass_window,
        geometry::MeshBuffers,
        swapchain,
        sync::{create_fence, create_semaphore, wait_and_reset},
    },
};

use size_dependent::SizeDependentState;
pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::Base>,
    pub vulkan: ManuallyDrop<wrap_vulkan::Base>,

    // the acquiring semaphores are used round-robin
    // because we need to supply a semaphore prior to knowing which frame to use
    last_used_acquire_semaphore: usize,
    window_semaphores_image_acquired: Vec<Semaphore>,
    // these are indexed by the result of acquiring
    window_semaphores_rendering_finished: Vec<Semaphore>,
    window_fences_rendering_finished: Vec<Fence>,
    window_command_buffers: Vec<CommandBuffer>,

    pub window_render_pass: RenderPass,
    size_dependent: SizeDependentState,
}

impl Drop for State {
    fn drop(&mut self) {
        self.vulkan.wait_idle().unwrap();

        unsafe {
            self.size_dependent.destroy(&self.vulkan);

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

pub struct PreRenderInfo {
    pub image_index: u32,
    image_acquired_semaphore: Semaphore,
}

impl State {
    pub fn pre_render(&mut self) -> Result<PreRenderInfo> {
        // prepare semaphore
        let image_acquired_semaphore =
            self.window_semaphores_image_acquired[self.last_used_acquire_semaphore];
        self.last_used_acquire_semaphore += 1;
        self.last_used_acquire_semaphore %= self.window_semaphores_image_acquired.len();

        // acuire image
        let (image_index, _suboptimal) = unsafe {
            self.size_dependent.swapchain.loader.acquire_next_image(
                self.size_dependent.swapchain.handle,
                std::u64::MAX, // don't timeout
                image_acquired_semaphore,
                ash::vk::Fence::default(),
            )
        }?;

        Ok(PreRenderInfo {
            image_index,
            image_acquired_semaphore,
        })
    }

    pub fn render(
        &self,
        pre_render_info: PreRenderInfo,
        pipeline_layout: PipelineLayout,
        pipeline: Pipeline,
        mesh: &MeshBuffers,
        descriptor_set: DescriptorSet,
    ) -> Result<()> {
        let PreRenderInfo {
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
        let window_extent = self.size_dependent.swapchain.extent;
        unsafe {
            let d = &self.vulkan.device;

            d.reset_command_buffer(command_buffer, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(command_buffer, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                command_buffer,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.window_render_pass)
                    .framebuffer(
                        self.size_dependent.swapchain.elements[image_index as usize].frame_buffer,
                    )
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

            let _suboptimal = self.size_dependent.swapchain.loader.queue_present(
                self.vulkan.queue,
                &PresentInfoKHR::builder()
                    .wait_semaphores(&[rendering_finished_semaphore])
                    .swapchains(&[self.size_dependent.swapchain.handle])
                    .image_indices(&[image_index]),
            )?;
        }

        Ok(())
    }

    pub fn resize(&mut self, window: &Window) -> Result<()> {
        self.vulkan.wait_idle()?;

        unsafe { self.size_dependent.destroy(&self.vulkan) };

        self.size_dependent = SizeDependentState::new(
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

        // Setup HMD

        let (session, mut frame_wait, mut frame_stream) = openxr.init_with_vulkan(&vulkan)?;

        //let hmd_extent = openxr.get_resolution()?;
        // TODO
        //let swapchain = wrap_openxr::get_swapchain(
        //&session,
        //hmd_extent,
        //format: Format,
        //)?;

        // Setup Window

        let window_render_pass = create_render_pass_window(&vulkan)?;

        let image_count = vulkan.get_image_count()?;

        let window_semaphores_image_acquired = (0..image_count)
            .into_iter()
            .map(|index| {
                Ok(create_semaphore(
                    &vulkan,
                    format!("WindowSemaphoreImageAcquired_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let window_semaphores_rendering_finished = (0..image_count)
            .into_iter()
            .map(|index| {
                Ok(create_semaphore(
                    &vulkan,
                    format!("WindowSemaphoreRenderingFinished_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let window_fences_rendering_finished = (0..image_count)
            .into_iter()
            .map(|index| {
                Ok(create_fence(
                    &vulkan,
                    true, // start in signaled state
                    format!("WindowFenceRenderingFinished_{}", index),
                )?)
            })
            .collect::<Result<_, Error>>()?;

        let size_dependent = SizeDependentState::new(
            &vulkan,
            window_render_pass,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;

        let window_command_buffers =
            vulkan.alloc_command_buffers(image_count, "WindowCommandBuffers".to_string())?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            last_used_acquire_semaphore: 0,
            window_semaphores_image_acquired,
            window_semaphores_rendering_finished,
            window_fences_rendering_finished,
            window_render_pass,
            window_command_buffers,
            size_dependent,
        })
    }
}
