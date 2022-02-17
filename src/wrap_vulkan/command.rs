use anyhow::Result;
use ash::vk::{
    CommandBuffer, CommandBufferAllocateInfo, CommandPool, CommandPoolCreateFlags,
    CommandPoolCreateInfo, Queue,
};

use super::Base;
pub struct CommandRelated {
    pub queue: Queue,
    pub pool: CommandPool,
}

impl CommandRelated {
    pub fn new(base: &Base) -> Result<Self> {
        let queue = unsafe { base.device.get_device_queue(base.queue_family_index, 0) };

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

        Ok(Self { queue, pool })
    }
}
