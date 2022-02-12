use std::fs::OpenOptions;

mod openxr;
mod vulkan;

pub struct State {
    openxr_state: openxr::State,
    vulkan_state: vulkan::State,
}

impl State {
    pub fn new() -> Self {
        log::info!("Creating new VRV state");

        let openxr_state = openxr::State::new();
        let vulkan_state = vulkan::State::new();

        Self {
            openxr_state,
            vulkan_state,
        }
    }
}
