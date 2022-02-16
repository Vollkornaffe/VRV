use anyhow::Result;
pub struct CommandRelated {

    queue:
}
impl CommandRelated {
    pub fn new() -> Result <Self> {


    }

}
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        let command_pool = unsafe {
            device.create_command_pool(
                &CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_family_index)
                    .flags(
                        CommandPoolCreateFlags::TRANSIENT
                            | CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    ),
                None,
            )
        }?;
