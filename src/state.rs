use std::mem::ManuallyDrop;

use anyhow::Result;
use ash::vk::{
    Buffer, ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBufferBeginInfo,
    CommandBufferResetFlags, Fence, ImageAspectFlags, ImageTiling, ImageUsageFlags,
    MemoryPropertyFlags, Pipeline, PipelineBindPoint, PipelineLayout, PipelineStageFlags,
    PresentInfoKHR, Rect2D, RenderPass, RenderPassBeginInfo, Semaphore, SubmitInfo,
    SubpassContents,
};
use winit::window::Window;

use crate::{
    wrap_openxr,
    wrap_vulkan::{
        self,
        command::CommandRelated,
        create_pipeline, create_pipeline_layout,
        sync::{create_fence, create_semaphore, wait_and_reset},
    },
};

pub struct State {
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,

    pub pipeline_layout: PipelineLayout,

    pub window_pipeline: Pipeline,
    pub window_swapchain: ManuallyDrop<wrap_vulkan::SwapchainRelated>,
    pub window_render_pass: RenderPass,
    pub window_depth_image: wrap_vulkan::DeviceImage,
    pub window_semaphore_image_acquired: Semaphore,
    pub window_semaphore_rendering_finished: Semaphore,
    pub window_fence_rendering_finished: Fence,

    pub command_related: CommandRelated,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
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
                .destroy_pipeline(self.window_pipeline, None);
            self.window_depth_image.drop(&self.vulkan.device);
            self.vulkan
                .device
                .destroy_render_pass(self.window_render_pass, None);
            self.window_swapchain.drop(&self.vulkan.device);
            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
        }
    }
}

impl State {
    pub fn render(&self) -> Result<()> {
        wait_and_reset(&self.vulkan, self.window_fence_rendering_finished)?;

        let (window_image_index, _suboptimal) = unsafe {
            self.window_swapchain.loader.acquire_next_image(
                self.window_swapchain.handle,
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
                        self.window_swapchain.elements[window_image_index as usize].frame_buffer,
                    )
                    .render_area(*Rect2D::builder().extent(self.window_swapchain.extent))
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
            d.cmd_bind_pipeline(cb, PipelineBindPoint::GRAPHICS, self.window_pipeline);
            //d.cmd_bind_vertex_buffers(cb, 0, &[TODO], &[0]);
            d.cmd_draw(cb, 3, 1, 0, 0);
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
            self.window_swapchain.loader.queue_present(
                self.command_related.queue,
                &PresentInfoKHR::builder()
                    .wait_semaphores(&[self.window_semaphore_rendering_finished])
                    .swapchains(&[self.window_swapchain.handle])
                    .image_indices(&[window_image_index]),
            )
        }?;

        Ok(())
    }

    pub fn new(window: &Window) -> Result<Self> {
        log::info!("Creating new VRV state");

        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::Base::new(window, &openxr)?;
        let mut window_swapchain =
            wrap_vulkan::SwapchainRelated::new(&window.inner_size(), &vulkan)?;

        let depth_format = vulkan.find_supported_depth_stencil_format()?;

        let window_render_pass = wrap_vulkan::create_render_pass_window(
            &vulkan,
            window_swapchain.surface_format.format,
            depth_format,
        )?;
        let window_depth_image = wrap_vulkan::DeviceImage::new(
            &vulkan,
            wrap_vulkan::device_image::DeviceImageSettings {
                extent: window_swapchain.extent,
                format: depth_format,
                tiling: ImageTiling::OPTIMAL,
                usage: ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                properties: MemoryPropertyFlags::DEVICE_LOCAL,
                aspect_flags: ImageAspectFlags::DEPTH,
                name: "DepthWindow".to_string(),
            },
        )?;

        window_swapchain.fill_elements(&vulkan, window_depth_image.view, window_render_pass)?;

        let pipeline_layout = create_pipeline_layout(&vulkan)?;
        let window_pipeline = create_pipeline(
            &vulkan,
            window_swapchain.extent,
            window_render_pass,
            pipeline_layout,
        )?;

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
            window_swapchain.image_count,
            1, /* TODO get number of HMD images */
        )?;

        Ok(Self {
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
            window_swapchain: ManuallyDrop::new(window_swapchain),
            window_render_pass,
            window_depth_image,
            pipeline_layout,
            window_pipeline,
            window_semaphore_image_acquired,
            window_semaphore_rendering_finished,
            window_fence_rendering_finished,
            command_related,
        })
    }
}
