use anyhow::Result;
use ash::vk::{
    CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandPool,
    CommandPoolCreateFlags, CommandPoolCreateInfo, Queue,
};

use super::Base;
pub struct CommandRelated {
    pub queue: Queue,
    pub pool: CommandPool,

    // TODO: come up with a better place
    // maybe a "Frame" abstraction, since there are buffers per frame in the swapchain
    pub window_buffers: Vec<CommandBuffer>,
    pub hmd_buffers: Vec<CommandBuffer>,
}

impl CommandRelated {
    pub fn new(base: &Base, buffer_count_window: u32, buffer_count_hmd: u32) -> Result<Self> {
        let queue = unsafe { base.device.get_device_queue(base.queue_family_index, 0) };
        base.name_object(queue, "GeneralPurposeQueue".to_string())?;

        let pool = unsafe {
            base.device.create_command_pool(
                &CommandPoolCreateInfo::builder()
                    .flags(
                        CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                            | CommandPoolCreateFlags::TRANSIENT,
                    )
                    .queue_family_index(base.queue_family_index),
                None,
            )
        }?;
        base.name_object(pool, "GeneralPurposeCommandPool".to_string())?;

        let alloc = |count| unsafe {
            base.device.allocate_command_buffers(
                &CommandBufferAllocateInfo::builder()
                    .command_pool(pool)
                    .level(CommandBufferLevel::PRIMARY)
                    .command_buffer_count(count),
            )
        };

        let window_buffers = alloc(buffer_count_window)?;
        let hmd_buffers = alloc(buffer_count_hmd)?;

        for (i, &cb) in window_buffers.iter().enumerate() {
            base.name_object(cb, format!("WindowCommandBuffer_{}", i))?;
        }
        for (i, &cb) in hmd_buffers.iter().enumerate() {
            base.name_object(cb, format!("HMDCommandBuffer_{}", i))?;
        }

        Ok(Self {
            queue,
            pool,
            window_buffers,
            hmd_buffers,
        })
    }
}
