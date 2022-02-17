use std::{ffi::CString, fs::File, path::Path};

use anyhow::Result;
use ash::{
    util::read_spv,
    vk::{
        BlendFactor, BlendOp, ColorComponentFlags, CompareOp, CullModeFlags, Extent2D, FrontFace,
        GraphicsPipelineCreateInfo, LogicOp, Offset2D, Pipeline, PipelineCache,
        PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
        PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayout,
        PipelineLayoutCreateInfo, PipelineMultisampleStateCreateInfo,
        PipelineRasterizationStateCreateInfo, PipelineShaderStageCreateInfo,
        PipelineViewportStateCreateInfo, PolygonMode, PrimitiveTopology, Rect2D, RenderPass,
        SampleCountFlags, ShaderModule, ShaderModuleCreateInfo, Viewport,
    },
};

use super::Base;

// later we can add set layouts and more
pub fn create_pipeline_layout(base: &Base) -> Result<PipelineLayout> {
    let layout = unsafe {
        base.device
            .create_pipeline_layout(&PipelineLayoutCreateInfo::builder(), None)
    }?;
    base.name_object(&layout, "FirstPipelineLayout".to_string())?;
    Ok(layout)
}

pub fn create_shader_module<P: AsRef<Path>>(
    base: &Base,
    path: P,
    name: String,
) -> Result<ShaderModule> {
    let module = unsafe {
        base.device.create_shader_module(
            &ShaderModuleCreateInfo::builder().code(&read_spv(&mut File::open(path)?)?),
            None,
        )
    }?;
    base.name_object(&module, name)?;
    Ok(module)
}

pub fn crate_pipeline(
    base: &Base,
    extent: Extent2D,
    render_pass: RenderPass,
    layout: PipelineLayout,
) -> Result<Pipeline> {
    let module_vert = create_shader_module(
        base,
        "examples/simple/shaders/vert.spv",
        "ShaderVert".to_string(),
    )?;
    let module_frag = create_shader_module(
        base,
        "examples/simple/shaders/frag.spv",
        "ShaderFrag".to_string(),
    )?;

    let entry_point = CString::new("main").unwrap();
    let pipeline = unsafe {
        base.device.create_graphics_pipelines(
            PipelineCache::default(),
            &[GraphicsPipelineCreateInfo::builder()
                .stages(&[
                    PipelineShaderStageCreateInfo::builder()
                        .stage(ash::vk::ShaderStageFlags::VERTEX)
                        .module(module_vert)
                        .name(&entry_point)
                        .build(),
                    PipelineShaderStageCreateInfo::builder()
                        .stage(ash::vk::ShaderStageFlags::FRAGMENT)
                        .module(module_frag)
                        .name(&entry_point)
                        .build(),
                ])
                .vertex_input_state(&vertex_input_info)
                .input_assembly_state(
                    &PipelineInputAssemblyStateCreateInfo::builder()
                        .topology(PrimitiveTopology::TRIANGLE_LIST)
                        .primitive_restart_enable(false),
                )
                .viewport_state(
                    &PipelineViewportStateCreateInfo::builder()
                        .viewports(&[Viewport::builder()
                            .x(0.0)
                            .y(0.0)
                            .width(extent.width as f32)
                            .height(extent.height as f32)
                            .min_depth(0.0)
                            .max_depth(1.0)
                            .build()])
                        .scissors(&[Rect2D::builder()
                            .offset(Offset2D { x: 0, y: 0 })
                            .extent(extent)
                            .build()]),
                )
                .rasterization_state(
                    &PipelineRasterizationStateCreateInfo::builder()
                        .depth_clamp_enable(false)
                        .rasterizer_discard_enable(false)
                        .polygon_mode(PolygonMode::FILL)
                        .line_width(1.0)
                        .cull_mode(CullModeFlags::BACK)
                        .front_face(FrontFace::COUNTER_CLOCKWISE)
                        .depth_bias_enable(false)
                        .depth_bias_constant_factor(0.0)
                        .depth_bias_clamp(0.0)
                        .depth_bias_slope_factor(0.0),
                )
                .multisample_state(
                    &PipelineMultisampleStateCreateInfo::builder()
                        .sample_shading_enable(false)
                        .rasterization_samples(SampleCountFlags::TYPE_1)
                        .min_sample_shading(1.0)
                        .alpha_to_coverage_enable(false)
                        .alpha_to_one_enable(false),
                )
                .color_blend_state(
                    &PipelineColorBlendStateCreateInfo::builder()
                        .logic_op_enable(false)
                        .logic_op(LogicOp::COPY)
                        .attachments(&[PipelineColorBlendAttachmentState::builder()
                            .color_write_mask(
                                ColorComponentFlags::R
                                    | ColorComponentFlags::G
                                    | ColorComponentFlags::B
                                    | ColorComponentFlags::A,
                            )
                            .blend_enable(false)
                            .src_color_blend_factor(BlendFactor::ONE)
                            .dst_color_blend_factor(BlendFactor::ZERO)
                            .color_blend_op(BlendOp::ADD)
                            .src_alpha_blend_factor(BlendFactor::ONE)
                            .dst_alpha_blend_factor(BlendFactor::ZERO)
                            .alpha_blend_op(BlendOp::ADD)
                            .build()])
                        .blend_constants([0.0, 0.0, 0.0, 0.0]),
                )
                .depth_stencil_state(
                    &PipelineDepthStencilStateCreateInfo::builder()
                        .depth_test_enable(true)
                        .depth_write_enable(true)
                        .depth_compare_op(CompareOp::LESS)
                        .depth_bounds_test_enable(false)
                        .min_depth_bounds(0.0)
                        .max_depth_bounds(1.0)
                        .stencil_test_enable(false),
                )
                .layout(layout)
                .render_pass(render_pass)
                .subpass(0)
                .build()],
            None,
        )
    }
    .map_err(|(_, e)| e)?[0];
    base.name_object(&layout, "FirstPipeline".to_string())?;

    unsafe {
        base.device.destroy_shader_module(module_vert, None);
        base.device.destroy_shader_module(module_frag, None);
    }

    Ok(pipeline)
}
