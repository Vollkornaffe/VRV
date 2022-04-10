use ash::vk::Extent2D;
use per_frame::PerFrame;
use simplelog::{Config, SimpleLogger};
use vk_shader_macros::include_glsl;
use vrv::{
    wrap_vulkan::{
        create_pipeline, create_pipeline_layout,
        pipeline::create_shader_module,
    },
    State,
};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod per_frame;

fn main() {
    let _ = SimpleLogger::init(log::LevelFilter::Warn, Config::default());

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(&window).unwrap();

    let (per_frame_buffers, descriptor_related) = PerFrame::new_vec(&state.vulkan).unwrap();

    const VERT: &[u32] = include_glsl!("shaders/example.vert");
    const FRAG: &[u32] = include_glsl!("shaders/example.frag");

    let module_vert = create_shader_module(&state.vulkan, VERT, "ShaderVert".to_string()).unwrap();
    let module_frag = create_shader_module(&state.vulkan, FRAG, "ShaderFrag".to_string()).unwrap();

    let pipeline_layout = create_pipeline_layout(
        &state.vulkan,
        descriptor_related.layout,
        "WindowPipelineLayout".to_string(),
    )
    .unwrap();

    let pipeline = create_pipeline(
        &state.vulkan,
        state.window_render_pass,
        pipeline_layout,
        module_vert,
        module_frag,
        Extent2D {
            width: window.inner_size().width,
            height: window.inner_size().height,
        },
        "WindowPipeline".to_string(),
    )
    .unwrap();

    unsafe {
        state.vulkan.device.destroy_shader_module(module_vert, None);
        state.vulkan.device.destroy_shader_module(module_frag, None);
    }

    // not sure if this is the way I want it...
    // it is an honest approach in the sense that the window is "on top"
    event_loop.run(move |event, _, control_flow| match event {
        Event::MainEventsCleared => {
            let pre_render_info = state.pre_render().unwrap();

            let current_frame = &per_frame_buffers[pre_render_info.image_index as usize];

            // TODO
            //state.set_camera_matrices_window().unwrap();

            state
                .render(
                    pre_render_info,
                    pipeline_layout,
                    pipeline,
                    &current_frame.mesh_buffers,
                    current_frame.descriptor_set,
                )
                .unwrap();

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
                WindowEvent::Resized(new_inner_size) => {
                    // TODO if the window is minimized, size is 0,0
                    // we need to make vulkan chill
                    log::info!("Resizing to {:?}", new_inner_size);
                    state.resize(&window).unwrap();
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor, // important for HUD and text in general
                    new_inner_size,
                } => {
                    log::info!("Changing scale to {}", scale_factor);
                    log::info!("Resizing to {:?}", new_inner_size);
                    state.resize(&window).unwrap();
                }
                _ => {}
            }
        }
        _ => {}
    })
}
