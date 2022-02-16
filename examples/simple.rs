use simplelog::{Config, SimpleLogger};
use vrv::State;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let _ = SimpleLogger::init(log::LevelFilter::Warn, Config::default());

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let _state = State::new(&window).unwrap();

    // not sure if this is the way I want it...
    // it is an honest approach in the sense that the window is "on top"
    event_loop.run(move |event, _, control_flow| match event {
        Event::MainEventsCleared => {
            // update and draw
            window.request_redraw();
        }
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(_physical_size) => {
                    // resize
                }
                WindowEvent::ScaleFactorChanged {
                    new_inner_size: _, ..
                } => {
                    // resize
                }
                _ => {}
            }
        }
        _ => {}
    })
}
