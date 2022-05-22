use anyhow::{bail, Error, Result};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use ash::vk::{
    self, CommandBuffer, DescriptorSet, DescriptorType, DynamicState, Extent2D, Fence, Semaphore,
    ShaderStageFlags,
};
use cgmath::{perspective, Deg, EuclideanSpace, Matrix4, Point3, SquareMatrix, Vector3};
use openxr::{EventDataBuffer, SessionState, ViewConfigurationType};
use simplelog::{Config, SimpleLogger};
use vk_shader_macros::include_glsl;
use vrv::{
    wrap_vulkan::{
        create_pipeline, create_pipeline_layout,
        descriptors::{DescriptorRelated, Usage},
        pipeline::create_shader_module,
        sync::{create_fence, create_semaphore},
    },
    Context,
};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::{
    buffer::{Buffer, UniformMatricesHMD, UniformMatricesWindow},
    camera::{fov_to_projection, pose_to_matrix_inverse, KeyMap, SphereCoords},
};

mod buffer;
mod camera;

#[derive(Debug)]
struct Double<UniformMatrices> {
    buffer: Buffer<UniformMatrices>,
    command: CommandBuffer,
    semaphore: Semaphore, // not used in HMD case
    fence: Fence,
}

impl<UniformMatrices> Double<UniformMatrices>
where
    UniformMatrices: std::fmt::Debug,
{
    fn create_front_and_back(context: &Context, prefix: String) -> Result<[Self; 2]> {
        Ok(["Front, Back"]
            .iter()
            .map(|front_or_back| {
                let buffer = Buffer::<UniformMatrices>::new(
                    &context.vulkan,
                    format!("{}{}Resource", prefix, front_or_back),
                )?;
                let command = context.vulkan.alloc_command_buffers(
                    1,
                    format!("{}{}CommandBuffer", prefix, front_or_back),
                )?[0];
                let semaphore =
                    create_semaphore(&context.vulkan, format!("{}RenderingFinished", prefix))?;
                let fence = create_fence(
                    &context.vulkan,
                    true,
                    format!("{}RenderingFinished", prefix),
                )?;

                Ok(Double::<UniformMatrices> {
                    buffer,
                    command,
                    semaphore,
                    fence,
                })
            })
            .collect::<Result<Vec<Self>>>()?
            .try_into()
            .unwrap())
    }

    fn make_descriptors(
        context: &Context,
        front_back: &[Self; 2],
        prefix: String,
    ) -> Result<(DescriptorRelated, Vec<DescriptorSet>)> {
        DescriptorRelated::new_with_sets(
            &context.vulkan,
            [(
                0,
                (DescriptorType::UNIFORM_BUFFER, ShaderStageFlags::VERTEX),
            )]
            .into(),
            &front_back
                .iter()
                .map(|front_or_back| front_or_back.buffer.matrix_buffer.handle())
                .map(|handle| [(0, Usage::Buffer(handle))].into())
                .collect::<Vec<_>>(),
            format!("{}Descriptor", prefix),
        )
    }
}

fn main() {
    let _ = SimpleLogger::init(log::LevelFilter::Warn, Config::default());

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut context = Context::new(&window).unwrap();

    let hmd_front_back =
        Double::<UniformMatricesHMD>::create_front_and_back(&context, "HMD".to_string()).unwrap();
    let (hmd_descriptor, hmd_descriptor_sets) = Double::<UniformMatricesHMD>::make_descriptors(
        &context,
        &hmd_front_back,
        "HMD".to_string(),
    )
    .unwrap();

    let window_front_back =
        Double::<UniformMatricesWindow>::create_front_and_back(&context, "Window".to_string())
            .unwrap();
    let (window_descriptor, window_descriptor_sets) =
        Double::<UniformMatricesWindow>::make_descriptors(
            &context,
            &window_front_back,
            "Window".to_string(),
        )
        .unwrap();

    const HMD_VERT: &[u32] = include_glsl!("shaders/example_hmd.vert");
    const HMD_FRAG: &[u32] = include_glsl!("shaders/example_hmd.frag");

    const WINDOW_VERT: &[u32] = include_glsl!("shaders/example_window.vert");
    const WINDOW_FRAG: &[u32] = include_glsl!("shaders/example_window.frag");

    let hmd_module_vert =
        create_shader_module(&context.vulkan, HMD_VERT, "HMDShaderVert".to_string()).unwrap();
    let hmd_module_frag =
        create_shader_module(&context.vulkan, HMD_FRAG, "HMDShaderFrag".to_string()).unwrap();

    let window_module_vert =
        create_shader_module(&context.vulkan, WINDOW_VERT, "WindowShaderVert".to_string()).unwrap();
    let window_module_frag =
        create_shader_module(&context.vulkan, WINDOW_FRAG, "WindowShaderFrag".to_string()).unwrap();

    let hmd_pipeline_layout = create_pipeline_layout(
        &context.vulkan,
        hmd_descriptor.layout,
        "HMDPipelineLayout".to_string(),
    )
    .unwrap();

    let hmd_pipeline = create_pipeline(
        &context.vulkan,
        context.hmd.render_pass,
        hmd_pipeline_layout,
        hmd_module_vert,
        hmd_module_frag,
        context.openxr.get_resolution().unwrap(),
        &[], // no dynamic state for now
        "HMDPipeline".to_string(),
    )
    .unwrap();

    let window_pipeline_layout = create_pipeline_layout(
        &context.vulkan,
        window_descriptor.layout,
        "WindowPipelineLayout".to_string(),
    )
    .unwrap();
    let window_pipeline = create_pipeline(
        &context.vulkan,
        context.window.render_pass,
        window_pipeline_layout,
        window_module_vert,
        window_module_frag,
        Extent2D {
            width: window.inner_size().width,
            height: window.inner_size().height,
        },
        &[DynamicState::VIEWPORT, DynamicState::SCISSOR], // allow for resize
        "WindowPipeline".to_string(),
    )
    .unwrap();

    unsafe {
        context
            .vulkan
            .device
            .destroy_shader_module(hmd_module_vert, None);
        context
            .vulkan
            .device
            .destroy_shader_module(hmd_module_frag, None);

        context
            .vulkan
            .device
            .destroy_shader_module(window_module_vert, None);
        context
            .vulkan
            .device
            .destroy_shader_module(window_module_frag, None);
    }

    let mut spherical_coords = SphereCoords::new();

    let mut pressed_keys: HashSet<VirtualKeyCode> = HashSet::new();

    // Handle interrupts gracefully
    let ctrlc = Arc::new(AtomicBool::new(false));
    {
        let r = ctrlc.clone();
        ctrlc::set_handler(move || {
            r.store(true, Ordering::Relaxed);
        })
        .expect("setting Ctrl-C handler");
    }

    let mut xr_event_storage = EventDataBuffer::new();
    let mut xr_session_running = false;
    let mut xr_focused = false;

    let mut hmd_flip_flop = 0;
    let mut window_flip_flop = 0;

    // not sure if this is the way I want it...
    // it is an honest approach in the sense that the window is "on top"
    event_loop.run(move |event, _, control_flow| match event {
        Event::MainEventsCleared => {
            if ctrlc.load(Ordering::Relaxed) {
                log::warn!("Exiting through Ctrl-C");

                *control_flow = ControlFlow::Exit;

                match context.session.request_exit() {
                    Ok(()) => {}
                    Err(openxr::sys::Result::ERROR_SESSION_NOT_RUNNING) => {}
                    Err(e) => panic!("{}", e),
                }

                return;
            }

            // handle OpenXR events
            while let Some(event) = context
                .openxr
                .instance
                .poll_event(&mut xr_event_storage)
                .unwrap()
            {
                use openxr::Event::*;
                match event {
                    SessionStateChanged(e) => {
                        // Session state change is where we can begin and end sessions, as well as
                        // find quit messages!
                        log::warn!("entered state {:?}", e.state());
                        xr_focused = false;
                        match e.state() {
                            SessionState::READY => {
                                context
                                    .session
                                    .begin(ViewConfigurationType::PRIMARY_STEREO)
                                    .unwrap();
                                xr_session_running = true;
                            }
                            SessionState::STOPPING => {
                                context.session.end().unwrap();
                                xr_session_running = false;
                            }
                            SessionState::FOCUSED => {
                                xr_focused = true;
                            }
                            SessionState::EXITING | SessionState::LOSS_PENDING => {
                                *control_flow = ControlFlow::Exit;
                                return;
                            }
                            _ => {}
                        }
                    }
                    InstanceLossPending(_) => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    EventsLost(e) => {
                        log::error!("lost {} events", e.lost_event_count());
                    }
                    _ => {}
                }
            }

            let hmd_pre_render_info = context.pre_render_hmd().unwrap();
            if hmd_pre_render_info.image_index.is_some() {
                context
                    .record_hmd(
                        hmd_pre_render_info,
                        hmd_pipeline_layout,
                        hmd_pipeline,
                        &hmd_front_back[hmd_flip_flop].buffer.mesh_buffers,
                        hmd_descriptor_sets[hmd_flip_flop],
                        hmd_front_back[hmd_flip_flop].command,
                        hmd_front_back[hmd_flip_flop].fence,
                    )
                    .unwrap();
                let views = context
                    .get_views(hmd_pre_render_info.frame_state.predicted_display_time)
                    .unwrap();

                hmd_front_back[hmd_flip_flop]
                    .buffer
                    .matrix_buffer
                    .write(&[UniformMatricesHMD {
                        model: Matrix4::identity(),
                        view_left: pose_to_matrix_inverse(views[0].pose),
                        view_right: pose_to_matrix_inverse(views[1].pose),
                        proj_left: fov_to_projection(views[0].fov),
                        proj_right: fov_to_projection(views[1].fov),
                    }]);

                context
                    .submit_hmd(
                        hmd_pre_render_info,
                        &views,
                        hmd_front_back[hmd_flip_flop].command,
                        hmd_front_back[hmd_flip_flop].fence,
                    )
                    .unwrap();

                hmd_flip_flop += 1;
                hmd_flip_flop %= 2;
            }

            let window_pre_render_info = context.pre_render_window().unwrap();
            spherical_coords.update(
                &pressed_keys
                    .iter()
                    .map(|&k| k.into())
                    .collect::<Vec<KeyMap>>(),
            );

            window_front_back[window_flip_flop]
                .buffer
                .matrix_buffer
                .write(&[UniformMatricesWindow {
                    model: Matrix4::identity(),
                    view: Matrix4::look_at_rh(
                        spherical_coords.to_coords(),
                        Point3::origin(),
                        Vector3::unit_y(),
                    ),
                    proj: {
                        // undo y inversion
                        let mut tmp = perspective(
                            Deg(45.0),
                            window.inner_size().width as f32 / window.inner_size().height as f32,
                            0.1,
                            100.0,
                        );
                        tmp[1][1] *= -1.0;
                        tmp
                    },
                }]);

            context
                .render_window(
                    window_pre_render_info,
                    window_pipeline_layout,
                    window_pipeline,
                    &window_front_back[window_flip_flop].buffer.mesh_buffers,
                    window_descriptor_sets[window_flip_flop],
                    window_front_back[window_flip_flop].command,
                    window_front_back[window_flip_flop].fence,
                    window_front_back[window_flip_flop].semaphore,
                )
                .unwrap();

            window.request_redraw();

            window_flip_flop += 1;
            window_flip_flop %= 2;
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
                    context.resize(&window).unwrap();
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor, // important for HUD and text in general
                    new_inner_size,
                } => {
                    log::info!("Changing scale to {}", scale_factor);
                    log::info!("Resizing to {:?}", new_inner_size);
                    context.resize(&window).unwrap();
                }
                // record key presses
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state,
                            virtual_keycode: Some(code),
                            ..
                        },
                    ..
                } => {
                    _ = match state {
                        ElementState::Pressed => pressed_keys.insert(*code),
                        ElementState::Released => pressed_keys.remove(code),
                    }
                }
                _ => {}
            }
        }
        _ => {}
    })
}
