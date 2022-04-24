use crate::{wrap_vulkan::sync::wait_and_reset, State};
use anyhow::Result;
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBufferBeginInfo,
    CommandBufferResetFlags, PipelineStageFlags, Rect2D, RenderPassBeginInfo, SubmitInfo,
    SubpassContents,
};
use openxr::{
    CompositionLayerProjection, CompositionLayerProjectionView, Duration, EnvironmentBlendMode,
    Extent2Di, Offset2Di, Rect2Di, SwapchainSubImage, ViewConfigurationType,
};

use super::PreRenderInfoHMD;

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

        let rendering_finished_fence = self.hmd_fences_rendering_finished[image_index as usize];
        let command_buffer = self.hmd_command_buffers[image_index as usize];
        let frame_buffer = self.hmd_swapchain.elements[image_index as usize].frame_buffer;
        let extent = self.hmd_swapchain.extent;

        // wait for rendering operations
        wait_and_reset(&self.vulkan, rendering_finished_fence)?;

        unsafe {
            let d = &self.vulkan.device;

            d.reset_command_buffer(command_buffer, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(command_buffer, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                command_buffer,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.hmd_render_pass)
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

            // TODO bind pipeline
            // TODO bind descriptor set
            // TODO bind vertex & index buffer
            // TODO draw

            d.cmd_end_render_pass(command_buffer);
            d.end_command_buffer(command_buffer)?;
        }

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

        unsafe {
            self.vulkan.device.queue_submit(
                self.vulkan.queue,
                &[SubmitInfo::builder()
                    .command_buffers(&[command_buffer])
                    .build()],
                rendering_finished_fence,
            )?;
        }

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
}
