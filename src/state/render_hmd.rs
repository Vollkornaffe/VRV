use crate::{wrap_vulkan::sync::wait_and_reset, State};
use anyhow::Result;
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

        // wait for rendering operations
        wait_and_reset(
            &self.vulkan,
            self.hmd_fences_rendering_finished[image_index as usize],
        )?;

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
}
