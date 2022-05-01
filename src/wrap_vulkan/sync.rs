use anyhow::Result;
use ash::vk::{Fence, FenceCreateFlags, FenceCreateInfo, Semaphore, SemaphoreCreateInfo};

use super::Context;

pub fn create_semaphore(context: &Context, name: String) -> Result<Semaphore> {
    let semaphore = unsafe {
        context
            .device
            .create_semaphore(&SemaphoreCreateInfo::builder(), None)
    }?;
    context.name_object(semaphore, name)?;
    Ok(semaphore)
}

pub fn create_fence(context: &Context, signaled: bool, name: String) -> Result<Fence> {
    let fence = unsafe {
        context.device.create_fence(
            &FenceCreateInfo::builder().flags(if signaled {
                FenceCreateFlags::SIGNALED
            } else {
                FenceCreateFlags::default()
            }),
            None,
        )
    }?;
    context.name_object(fence, name)?;
    Ok(fence)
}

pub fn wait_and_reset(context: &Context, fence: Fence) -> Result<()> {
    unsafe {
        context.device.wait_for_fences(
            &[fence],
            true,          // wait all
            std::u64::MAX, // don't timeout
        )
    }?;
    unsafe { context.device.reset_fences(&[fence]) }?;
    Ok(())
}
