mod resize;
use resize::ResizableWindowState;

use std::mem::ManuallyDrop;

use anyhow::Result;
use ash::vk::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBufferBeginInfo,
    CommandBufferResetFlags, Extent2D, Fence, IndexType, PipelineBindPoint, PipelineLayout,
    PipelineStageFlags, PresentInfoKHR, Rect2D, RenderPass, RenderPassBeginInfo, Semaphore,
    SubmitInfo, SubpassContents,
};

use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self,
        command::CommandRelated,
        create_pipeline_layout,
        geometry::{MappedMesh, Mesh},
        sync::{create_fence, create_semaphore, wait_and_reset},
    },
};

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub pipeline_layout: PipelineLayout,

    pub command_related: CommandRelated,
    pub debug_mapped_mesh: MappedMesh,

    pub window_render_pass: RenderPass,
    pub window_semaphore_image_acquired: Semaphore,
    pub window_semaphore_rendering_finished: Semaphore,
    pub window_fence_rendering_finished: Fence,

    pub resizable_window_state: ResizableWindowState,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.vulkan
                .device
                .queue_wait_idle(self.command_related.queue)
                .unwrap();

            self.resizable_window_state.destroy(&self.vulkan);
            self.debug_mapped_mesh.destroy(&self.vulkan);
            // takes care of command buffers
            self.vulkan
                .device
                .destroy_command_pool(self.command_related.pool, None);
            self.vulkan
                .device
                .destroy_semaphore(self.window_semaphore_image_acquired, None);
            self.vulkan
                .device
                .destroy_semaphore(self.window_semaphore_rendering_finished, None);
            self.vulkan
                .device
                .destroy_fence(self.window_fence_rendering_finished, None);
            self.vulkan
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.vulkan
                .device
                .destroy_render_pass(self.window_render_pass, None);
            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
        }
    }
}

impl State {
    pub fn render(&self) -> Result<()> {
        wait_and_reset(&self.vulkan, self.window_fence_rendering_finished)?;
        let window_swapchain = &self.resizable_window_state.swapchain;

        let (window_image_index, _suboptimal) = unsafe {
            window_swapchain.loader.acquire_next_image(
                window_swapchain.handle,
                std::u64::MAX, // don't timeout
                self.window_semaphore_image_acquired,
                ash::vk::Fence::default(),
            )
        }?;

        unsafe {
            let d = &self.vulkan.device;
            let cb = self.command_related.window_buffers[window_image_index as usize];

            d.reset_command_buffer(cb, CommandBufferResetFlags::RELEASE_RESOURCES)?;
            d.begin_command_buffer(cb, &CommandBufferBeginInfo::builder())?;
            d.cmd_begin_render_pass(
                cb,
                &RenderPassBeginInfo::builder()
                    .render_pass(self.window_render_pass)
                    .framebuffer(
                        window_swapchain.elements[window_image_index as usize].frame_buffer,
                    )
                    .render_area(*Rect2D::builder().extent(window_swapchain.extent))
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
            d.cmd_bind_pipeline(
                cb,
                PipelineBindPoint::GRAPHICS,
                self.resizable_window_state.pipeline,
            );
            d.cmd_bind_vertex_buffers(cb, 0, &[self.debug_mapped_mesh.vertex_buffer()], &[0]);
            d.cmd_bind_index_buffer(
                cb,
                self.debug_mapped_mesh.index_buffer(),
                0,
                IndexType::UINT32,
            );
            d.cmd_draw_indexed(cb, self.debug_mapped_mesh.num_indices() as u32, 1, 0, 0, 0);
            d.cmd_end_render_pass(cb);
            d.end_command_buffer(cb)?;
        }

        unsafe {
            self.vulkan.device.queue_submit(
                self.command_related.queue,
                &[SubmitInfo::builder()
                    .command_buffers(&[
                        self.command_related.window_buffers[window_image_index as usize]
                    ])
                    .wait_semaphores(&[self.window_semaphore_image_acquired])
                    .wait_dst_stage_mask(&[PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                    .signal_semaphores(&[self.window_semaphore_rendering_finished])
                    .build()],
                self.window_fence_rendering_finished,
            )
        }?;

        let _suboptimal = unsafe {
            window_swapchain.loader.queue_present(
                self.command_related.queue,
                &PresentInfoKHR::builder()
                    .wait_semaphores(&[self.window_semaphore_rendering_finished])
                    .swapchains(&[self.resizable_window_state.swapchain.handle])
                    .image_indices(&[window_image_index]),
            )
        }?;

        Ok(())
    }

    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::Base::new(window, &openxr)?;

        let image_count = vulkan.get_image_count()?;

        let window_render_pass = wrap_vulkan::create_render_pass_window(&vulkan)?;

        let pipeline_layout = create_pipeline_layout(&vulkan)?;

        let window_semaphore_image_acquired =
            create_semaphore(&vulkan, "WindowSemaphoreImageAcquired".to_string())?;
        let window_semaphore_rendering_finished =
            create_semaphore(&vulkan, "WindowSemaphoreRenderingFinished".to_string())?;
        let window_fence_rendering_finished = create_fence(
            &vulkan,
            true, // we start with finished rendering
            "WindowFenceRenderingFinihsed".to_string(),
        )?;

        let command_related = CommandRelated::new(
            &vulkan,
            image_count,
            1, /* TODO get number of HMD images */
        )?;

        let debug_mapped_mesh =
            MappedMesh::new(&vulkan, Mesh::debug_triangle(), "DebugMesh".to_string())?;

        let resizable_window_state = ResizableWindowState::new(
            &vulkan,
            window_render_pass,
            pipeline_layout,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            pipeline_layout,
            command_related,
            debug_mapped_mesh,
            window_render_pass,
            window_semaphore_image_acquired,
            window_semaphore_rendering_finished,
            window_fence_rendering_finished,
            resizable_window_state,
        })
    }

    pub fn resize(&mut self, window: &Window) -> Result<()> {
        unsafe {
            self.vulkan
                .device
                .queue_wait_idle(self.command_related.queue)
        }?;

        unsafe { self.resizable_window_state.destroy(&self.vulkan) };

        self.resizable_window_state = ResizableWindowState::new(
            &self.vulkan,
            self.window_render_pass,
            self.pipeline_layout,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
        )?;

        Ok(())
    }
}
