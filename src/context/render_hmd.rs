use crate::{
    wrap_vulkan::{geometry::MeshBuffers, sync::wait_and_reset},
    Context,
};
use anyhow::{Error, Result};
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBuffer, CommandBufferBeginInfo,
    CommandBufferResetFlags, DescriptorSet, Fence, IndexType, Pipeline, PipelineBindPoint,
    PipelineLayout, Rect2D, RenderPassBeginInfo, SubmitInfo, SubpassContents,
};

use openxr::{
    CompositionLayerProjection, CompositionLayerProjectionView, Duration, EnvironmentBlendMode,
    Extent2Di, Offset2Di, Rect2Di, SwapchainSubImage, View,
};

use super::PreRenderInfoHMD;

impl Context {
    pub fn pre_render_hmd(&mut self) -> Result<PreRenderInfoHMD> {
        let frame_state = self.hmd.frame_wait.wait()?;
        self.hmd.frame_stream.begin()?;

        if !frame_state.should_render {
            self.hmd.frame_stream.end(
                frame_state.predicted_display_time,
                EnvironmentBlendMode::OPAQUE,
                &[],
            )?;
        }

        let image_index = if frame_state.should_render {
            Some(self.hmd.swapchain.swapchain.acquire_image()?)
        } else {
            None
        };

        Ok(PreRenderInfoHMD {
            image_index,
            frame_state,
        })
    }

    pub fn record_hmd(
        &mut self,
        pre_render_info: PreRenderInfoHMD,
        pipeline_layout: PipelineLayout,
        pipeline: Pipeline,
        mesh: &MeshBuffers,
        descriptor_set: DescriptorSet,
        command_buffer: CommandBuffer,
        rendering_finished_fence: Fence,
    ) -> Result<()> {
        let PreRenderInfoHMD { image_index, .. } = pre_render_info;

        let image_index = image_index.ok_or(Error::msg("Shouldn't render, says OpenXR"))?;

        // Wait until the image is available to render to. The compositor could still be
        // reading from it.
        self.hmd
            .swapchain
            .swapchain
            .wait_image(Duration::INFINITE)?;

        let frame_buffer = self.hmd.swapchain.elements[image_index as usize].frame_buffer;
        let extent = self.hmd.swapchain.extent;

        // wait for rendering operations
        wait_and_reset(&self.vulkan, rendering_finished_fence)?;

        unsafe {
            let d = &self.vulkan.device;

            d.reset_command_buffer(command_buffer, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(command_buffer, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                command_buffer,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.hmd.render_pass)
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
        }
        Ok(())
    }

    pub fn submit_hmd(
        &mut self,
        pre_render_info: PreRenderInfoHMD,
        views: &[View; 2],
        command_buffer: CommandBuffer,
        rendering_finished_fence: Fence,
    ) -> Result<()> {
        let PreRenderInfoHMD {
            image_index,
            frame_state,
        } = pre_render_info;

        let image_index = image_index.ok_or(Error::msg("Shouldn't render, says OpenXR"))?;

        unsafe {
            self.vulkan.device.queue_submit(
                self.vulkan.queue,
                &[SubmitInfo::builder()
                    .command_buffers(&[command_buffer])
                    .build()],
                rendering_finished_fence,
            )?;
        }

        self.hmd.swapchain.swapchain.release_image()?;

        self.hmd.frame_stream.end(
            frame_state.predicted_display_time,
            EnvironmentBlendMode::OPAQUE,
            &[&CompositionLayerProjection::new()
                .space(&self.hmd.stage)
                .views(
                    &views
                        .iter()
                        .enumerate()
                        .map(|(i, view)| {
                            CompositionLayerProjectionView::new()
                                .pose(view.pose)
                                .fov(view.fov)
                                .sub_image(
                                    SwapchainSubImage::new()
                                        .swapchain(&self.hmd.swapchain.swapchain)
                                        .image_array_index(i as u32)
                                        .image_rect(Rect2Di {
                                            offset: Offset2Di::default(),
                                            extent: Extent2Di {
                                                width: self.hmd.swapchain.extent.width as i32,
                                                height: self.hmd.swapchain.extent.height as i32,
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
