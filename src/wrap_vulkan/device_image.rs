use anyhow::Result;
use ash::vk::{
    DeviceMemory, Extent2D, Extent3D, Format, Image, ImageAspectFlags, ImageCreateInfo,
    ImageLayout, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags, ImageView,
    ImageViewCreateInfo, ImageViewType, MemoryAllocateInfo, MemoryPropertyFlags, SampleCountFlags,
    SharingMode,
};

use super::Context;

pub struct DeviceImage {
    pub image: Image,
    pub memory: DeviceMemory,
    pub view: ImageView,
}

pub struct DeviceImageSettings {
    pub extent: Extent2D,
    pub format: Format,
    pub tiling: ImageTiling,
    pub usage: ImageUsageFlags,
    pub properties: MemoryPropertyFlags,
    pub aspect_flags: ImageAspectFlags,
    pub layer_count: u32, // 2 for hmd
    pub name: String,
}

impl DeviceImage {
    pub fn new_view(
        context: &Context,
        image: Image,
        format: Format,
        aspect_flags: ImageAspectFlags,
        layer_count: u32,
        name: String,
    ) -> Result<ImageView> {
        let view = unsafe {
            context.device.create_image_view(
                &ImageViewCreateInfo::builder()
                    .image(image)
                    .view_type(if layer_count == 1 {
                        ImageViewType::TYPE_2D
                    } else {
                        ImageViewType::TYPE_2D_ARRAY
                    })
                    .format(format)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .aspect_mask(aspect_flags)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(layer_count)
                            .build(),
                    ),
                None,
            )
        }?;
        context.name_object(view, name)?;
        Ok(view)
    }

    pub fn new(context: &Context, settings: DeviceImageSettings) -> Result<Self> {
        let image = unsafe {
            context.device.create_image(
                &ImageCreateInfo::builder()
                    .image_type(ImageType::TYPE_2D)
                    .extent(Extent3D {
                        width: settings.extent.width,
                        height: settings.extent.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(settings.layer_count)
                    .format(settings.format)
                    .tiling(settings.tiling)
                    .initial_layout(ImageLayout::UNDEFINED)
                    .usage(settings.usage)
                    .sharing_mode(SharingMode::EXCLUSIVE)
                    .samples(SampleCountFlags::TYPE_1),
                None,
            )
        }?;
        context.name_object(image, format!("{}Image", settings.name.clone()))?;

        let memory_requirements = unsafe { context.device.get_image_memory_requirements(image) };
        let memory = unsafe {
            context.device.allocate_memory(
                &MemoryAllocateInfo::builder()
                    .allocation_size(memory_requirements.size)
                    .memory_type_index(context.find_memory_type_index(
                        MemoryPropertyFlags::from_raw(memory_requirements.memory_type_bits),
                        settings.properties,
                    )?),
                None,
            )?
        };
        context.name_object(memory, format!("{}Memory", settings.name.clone()))?;

        unsafe { context.device.bind_image_memory(image, memory, 0) }?;

        let view = Self::new_view(
            context,
            image,
            settings.format,
            settings.aspect_flags,
            settings.layer_count,
            format!("{}View", settings.name.clone()),
        )?;

        Ok(Self {
            image,
            memory,
            view,
        })
    }

    pub unsafe fn destroy(&self, context: &Context) {
        context.device.destroy_image_view(self.view, None);
        context.device.destroy_image(self.image, None);
        context.device.free_memory(self.memory, None);
    }
}
