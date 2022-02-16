use anyhow::Result;
use ash::vk::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp,
    Format, FormatFeatureFlags, ImageLayout, ImageTiling, PipelineBindPoint, PipelineStageFlags,
    RenderPass, RenderPassCreateInfo, SampleCountFlags, SubpassDependency, SubpassDescription,
    SUBPASS_EXTERNAL,
};

use super::Base;

fn attachments(base: &Base, swapchain_format: Format) -> Result<[AttachmentDescription; 2]> {
    Ok([
        AttachmentDescription::builder()
            .format(swapchain_format)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::PRESENT_SRC_KHR)
            .build(),
        AttachmentDescription::builder()
            .format(base.find_supported_format(
                vec![
                    Format::D32_SFLOAT,
                    Format::D32_SFLOAT_S8_UINT,
                    Format::D24_UNORM_S8_UINT,
                ],
                ImageTiling::OPTIMAL,
                FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
            )?)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build(),
    ])
}

fn subpasses() -> [SubpassDescription; 1] {
    [SubpassDescription::builder()
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
        .build()]
}

fn dependencies() -> [SubpassDependency; 1] {
    [SubpassDependency::builder()
        .src_subpass(SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE | AccessFlags::COLOR_ATTACHMENT_WRITE,
        )
        .build()]
}

pub fn create_window_render_pass(base: &Base, swapchain_format: Format) -> Result<RenderPass> {
    let render_pass = unsafe {
        base.device.create_render_pass(
            &RenderPassCreateInfo::builder()
                .attachments(&attachments(base, swapchain_format)?)
                .subpasses(&subpasses())
                .dependencies(&dependencies()),
            None,
        )
    }?;
    base.name_object(&render_pass, "RenderPassWindow".to_string());
    Ok(render_pass)
}
