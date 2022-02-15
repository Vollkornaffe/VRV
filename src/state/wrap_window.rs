use std::os::raw::c_char;

use anyhow::Result;
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

    pub fn get_instance_extensions(&self) -> Result<Vec<*const c_char>> {
        Ok(ash_window::enumerate_required_extensions(&self.window)?
            .iter()
            .map(|x| x.as_ptr())
            .collect())
    }
}
