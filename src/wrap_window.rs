use std::ffi::CString;

use anyhow::Result;
use ash::extensions::khr::Swapchain;
use winit::{
    event_loop::EventLoop,
    platform::windows::EventLoopExtWindows,
    window::{Window, WindowBuilder},
};

pub struct State {
    pub event_loop: EventLoop<() /* user event type */>,
    pub window: Window,
}

impl State {
    pub fn new() -> Self {
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        Self { event_loop, window }
    }

    pub fn get_instance_extensions(&self) -> Result<Vec<CString>> {
        Ok(ash_window::enumerate_required_extensions(&self.window)?
            .iter()
            .map(|&x| x.into())
            .collect())
    }

    pub fn get_device_extensions(&self) -> Vec<CString> {
        vec![Swapchain::name().into()]
    }
}
