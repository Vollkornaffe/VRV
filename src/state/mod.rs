use anyhow::Result;

pub mod wrap_openxr;
pub mod wrap_vulkan;
pub mod wrap_window;

pub struct State {
    window: wrap_window::State,
    openxr: wrap_openxr::State,
    vulkan: wrap_vulkan::State,
}

impl State {
    pub fn new() -> Result<Self> {
        log::info!("Creating new VRV state");

        let window = wrap_window::State::new();
        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::State::new(&window, &openxr)?;

        Ok(Self {
            window,
            openxr,
            vulkan,
        })
    }
}
