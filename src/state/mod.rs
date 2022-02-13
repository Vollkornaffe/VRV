use anyhow::Result;

mod wrap_openxr;
mod wrap_vulkan;
mod wrap_winit;

pub struct State {
    window: wrap_winit::State,
    openxr: wrap_openxr::State,
    vulkan: wrap_vulkan::State,
}

impl State {
    pub fn new() -> Result<Self> {
        log::info!("Creating new VRV state");
        Ok(Self {
            window: wrap_winit::State::new(),
            openxr: wrap_openxr::State::new()?,
            vulkan: wrap_vulkan::State::new(),
        })
    }
}
