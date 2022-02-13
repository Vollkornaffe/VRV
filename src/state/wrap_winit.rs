use winit::{
    event_loop::EventLoop,
    platform::windows::EventLoopExtWindows,
    window::{Window, WindowBuilder},
};

pub struct State {
    event_loop: EventLoop<() /* user event type */>,
    window: Window,
}

impl State {
    pub fn new() -> Self {
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        Self { event_loop, window }
    }
}
