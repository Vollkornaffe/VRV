use anyhow::Result;

pub mod wrap_openxr;
pub mod wrap_vulkan;
pub mod wrap_winit;

pub struct State {
    window: wrap_winit::State,
    openxr: wrap_openxr::State,
    vulkan: wrap_vulkan::State,
}

impl State {
    pub fn new() -> Result<Self> {
        log::info!("Creating new VRV state");

        let window = wrap_winit::State::new();
        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::State::new(&openxr)?;

        Ok(Self {
            window,
            openxr,
            vulkan,
        })
    }
}
