use crate::{
    wrap_vulkan::{geometry::MeshBuffers, sync::wait_and_reset},
    State,
};
use anyhow::Result;
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBufferBeginInfo,
    CommandBufferResetFlags, DescriptorSet, IndexType, Offset2D, Pipeline, PipelineBindPoint,
    PipelineLayout, PipelineStageFlags, PresentInfoKHR, Rect2D, RenderPassBeginInfo, SubmitInfo,
    SubpassContents, Viewport,
};

use super::PreRenderInfoWindow;

impl State {
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
        let frame_buffer = self.window_swapchain.elements[image_index as usize].frame_buffer;

        // waite before resetting cmd buffer
        wait_and_reset(&self.vulkan, rendering_finished_fence)?;

        // for convenience
        let extent = self.window_swapchain.extent;
        unsafe {
            let d = &self.vulkan.device;

            d.reset_command_buffer(command_buffer, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(command_buffer, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                command_buffer,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.window_render_pass)
                    .framebuffer(frame_buffer)
                    .render_area(*Rect2D::builder().extent(extent))
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
                    .width(extent.width as f32)
                    .height(extent.height as f32)
                    .min_depth(0.0)
                    .max_depth(1.0)
                    .build()],
            );
            d.cmd_set_scissor(
                command_buffer,
                0,
                &[Rect2D::builder()
                    .offset(Offset2D { x: 0, y: 0 })
                    .extent(extent)
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
                rendering_finished_fence,
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
}
