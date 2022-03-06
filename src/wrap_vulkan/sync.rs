use anyhow::Result;
use ash::vk::{Fence, FenceCreateFlags, FenceCreateInfo, Semaphore, SemaphoreCreateInfo};

use super::Base;

pub fn create_semaphore(base: &Base, name: String) -> Result<Semaphore> {
    let semaphore = unsafe {
        base.device
            .create_semaphore(&SemaphoreCreateInfo::builder(), None)
    }?;
    base.name_object(semaphore, name)?;
    Ok(semaphore)
}

pub fn create_fence(base: &Base, signaled: bool, name: String) -> Result<Fence> {
    let fence = unsafe {
        base.device.create_fence(
            &FenceCreateInfo::builder().flags(if signaled {
                FenceCreateFlags::SIGNALED
            } else {
                FenceCreateFlags::default()
            }),
            None,
        )
    }?;
    base.name_object(fence, name)?;
    Ok(fence)
}

pub fn wait_and_reset(base: &Base, fence: Fence) -> Result<()> {
    unsafe {
        base.device.wait_for_fences(
            &[fence],
            true,          // wait all
            std::u64::MAX, // don't timeout
        )
    }?;
    unsafe { base.device.reset_fences(&[fence]) }?;
    Ok(())
}
