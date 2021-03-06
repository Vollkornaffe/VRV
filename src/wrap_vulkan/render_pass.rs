use anyhow::Result;
use ash::vk::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp,
    ImageLayout, PipelineBindPoint, PipelineStageFlags, RenderPass, RenderPassCreateInfo,
    RenderPassMultiviewCreateInfo, SampleCountFlags, SubpassDependency, SubpassDescription,
    SUBPASS_EXTERNAL,
};

use super::Context;

pub fn create_render_pass_window(context: &Context) -> Result<RenderPass> {
    let render_pass = unsafe {
        context.device.create_render_pass(
            &RenderPassCreateInfo::builder()
                .attachments(&[
                    AttachmentDescription::builder()
                        .format(context.get_surface_format()?)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::STORE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        .final_layout(ImageLayout::PRESENT_SRC_KHR)
                        .build(),
                    AttachmentDescription::builder()
                        .format(context.find_supported_depth_stencil_format()?)
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
    context.name_object(render_pass, "RenderPassWindow".to_string())?;
    Ok(render_pass)
}

pub fn create_render_pass_hmd(context: &Context) -> Result<RenderPass> {
    // sets the 2 least significant bits
    let masks = [!(!0 << 2)];

    let render_pass = unsafe {
        context.device.create_render_pass(
            &RenderPassCreateInfo::builder()
                .attachments(&[
                    AttachmentDescription::builder()
                        .format(context.find_supported_color_format()?)
                        .samples(SampleCountFlags::TYPE_1)
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::STORE)
                        .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                        .initial_layout(ImageLayout::UNDEFINED)
                        // final layout isn't PRESENT_SRC_KHR
                        .final_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .build(),
                    AttachmentDescription::builder()
                        .format(context.find_supported_depth_stencil_format()?)
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
                    .src_access_mask(AccessFlags::empty())
                    .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .build()])
                // there is no next in the window swapchain
                .push_next(
                    &mut RenderPassMultiviewCreateInfo::builder()
                        .view_masks(&masks)
                        .correlation_masks(&masks),
                ),
            None,
        )
    }?;
    context.name_object(render_pass, "RenderPassHMD".to_string())?;

    Ok(render_pass)
}
