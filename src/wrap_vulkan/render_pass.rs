use anyhow::Result;
use ash::vk::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp,
    Format, FormatFeatureFlags, ImageLayout, ImageTiling, PipelineBindPoint, PipelineStageFlags,
    RenderPass, RenderPassCreateInfo, SampleCountFlags, SubpassDependency, SubpassDescription,
    SUBPASS_EXTERNAL, RenderPassMultiviewCreateInfo,
};

use super::Base;

pub fn create_render_pass_window(base: &Base) -> Result<RenderPass> {
    let color_format = base.get_surface_format()?;

    let render_pass = unsafe {
        base.device.create_render_pass(
            &RenderPassCreateInfo::builder()
                .attachments(&[
                    AttachmentDescription::builder()
                        .format(color_format)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::STORE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        .final_layout(ImageLayout::PRESENT_SRC_KHR)
                        .build(),
                    AttachmentDescription::builder()
                        .format(base.find_supported_depth_stencil_format()?)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::DONT_CARE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .build(),
                ])
                .subpasses(&[SubpassDescription::builder()
                    .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
                    .color_attachments(&[AttachmentReference::builder()
                        .attachment(0)
                        .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .build()])
                    .depth_stencil_attachment(
                        &AttachmentReference::builder()
                            .attachment(1)
                            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
                    )
                    .build()])
                .dependencies(&[SubpassDependency::builder()
                    .src_subpass(SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .build()]),
            None,
        )
    }?;
    base.name_object(render_pass, "RenderPassWindow".to_string())?;
    Ok(render_pass)
}

pub fn create_render_pass_hmd(base: &Base) -> Result<RenderPass> {

    // sets the 2 least significant bits
    let masks = [!(!0 << 2)];

    let render_pass = unsafe {
        base.device.create_render_pass(
            &RenderPassCreateInfo::builder()
                .attachments(&[
                    AttachmentDescription::builder()
                        .format(base.find_supported_color_format()?)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::STORE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        .final_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .build(),
                    AttachmentDescription::builder()
                        .format(base.find_supported_depth_stencil_format()?)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::DONT_CARE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .build(),
                ])
                .subpasses(&[SubpassDescription::builder()
                    .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
                    .color_attachments(&[AttachmentReference::builder()
                        .attachment(0)
                        .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .build()])
                    .depth_stencil_attachment(
                        &AttachmentReference::builder()
                            .attachment(1)
                            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
                    )
                    .build()])
                .dependencies(&[SubpassDependency::builder()
                    .src_subpass(SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .build()])
                    .push_next(&mut RenderPassMultiviewCreateInfo::builder().view_masks(&masks).correlation_masks(&masks)),
            None,
        )
    }?;
    base.name_object(render_pass, "RenderPassHMD".to_string())?;

    Ok(render_pass)
}
