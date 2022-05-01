use crate::wrap_vulkan::{
    buffers::MappedDeviceBuffer,
    device_image::{DeviceImageSettings, TransitionImageLayoutSettings},
    Context, DeviceImage,
};

use anyhow::Result;
use ash::vk::{
    AccessFlags, BufferImageCopy, Extent2D, Extent3D, Format, ImageAspectFlags, ImageLayout,
    ImageSubresourceLayers, ImageTiling, ImageUsageFlags, MemoryPropertyFlags, Offset3D,
    PipelineStageFlags,
};

pub fn create_texture(
    context: &Context,
    extent: Extent2D,
    data: &[u8],
    format: Format,
    aspect_mask: ImageAspectFlags,
    name: String,
) -> Result<DeviceImage> {
    let staging =
        MappedDeviceBuffer::<u8>::new_staging(context, data.len(), format!("{}Staging", name))?;
    staging.write(data);

    let texture = DeviceImage::new(
        context,
        DeviceImageSettings {
            extent,
            format,
            tiling: ImageTiling::OPTIMAL,
            usage: ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED,
            properties: MemoryPropertyFlags::DEVICE_LOCAL,
            aspect_mask,
            layer_count: 1,
            name: name.clone(),
        },
    )?;

    context.one_shot(
        |cmd| {
            texture.transition_layout(
                context,
                TransitionImageLayoutSettings {
                    image: texture.image,
                    layer: 1,
                    aspect_mask,
                    old_layout: ImageLayout::UNDEFINED,
                    new_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
                    src_access: AccessFlags::default(),
                    dst_access: AccessFlags::TRANSFER_WRITE,
                    src_stage: PipelineStageFlags::TOP_OF_PIPE,
                    dst_stage: PipelineStageFlags::TRANSFER,
                },
                cmd,
            );

            unsafe {
                context.device.cmd_copy_buffer_to_image(
                    cmd,
                    staging.handle(),
                    texture.image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[BufferImageCopy::builder()
                        .buffer_offset(0)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(
                            ImageSubresourceLayers::builder()
                                .aspect_mask(aspect_mask)
                                .mip_level(0)
                                .base_array_layer(0)
                                .layer_count(1)
                                .build(),
                        )
                        .image_offset(Offset3D { x: 0, y: 0, z: 0 })
                        .image_extent(Extent3D {
                            width: extent.width,
                            height: extent.height,
                            depth: 1,
                        })
                        .build()],
                );
            }

            texture.transition_layout(
                context,
                TransitionImageLayoutSettings {
                    image: texture.image,
                    layer: 1,
                    aspect_mask,
                    old_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    src_access: AccessFlags::TRANSFER_WRITE,
                    dst_access: AccessFlags::SHADER_READ,
                    src_stage: PipelineStageFlags::TRANSFER,
                    dst_stage: PipelineStageFlags::FRAGMENT_SHADER,
                },
                cmd,
            );

            Ok(())
        },
        format!("{}Transfer", name),
    )?;

    Ok(texture)
}
