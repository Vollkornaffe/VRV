use anyhow::Result;
use ash::{
    vk::{
        DeviceMemory, Extent2D, Extent3D, Format, Image, ImageAspectFlags, ImageCreateInfo,
        ImageLayout, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags, ImageView,
        ImageViewCreateInfo, ImageViewType, MemoryAllocateInfo, MemoryPropertyFlags,
        SampleCountFlags, SharingMode,
    },
    Device,
};

use super::Base;

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
    pub name: String,
}

impl DeviceImage {
    pub fn new(base: &Base, settings: DeviceImageSettings) -> Result<Self> {
        let image = unsafe {
            base.device.create_image(
                &ImageCreateInfo::builder()
                    .image_type(ImageType::TYPE_2D)
                    .extent(Extent3D {
                        width: settings.extent.width,
                        height: settings.extent.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .format(settings.format)
                    .tiling(settings.tiling)
                    .initial_layout(ImageLayout::UNDEFINED)
                    .usage(settings.usage)
                    .sharing_mode(SharingMode::EXCLUSIVE)
                    .samples(SampleCountFlags::TYPE_1),
                None,
            )
        }?;
        base.name_object(&image, format!("{}Image", settings.name.clone()))?;

        let memory_requirements = unsafe { base.device.get_image_memory_requirements(image) };
        let memory = unsafe {
            base.device.allocate_memory(
                &MemoryAllocateInfo::builder()
                    .allocation_size(memory_requirements.size)
                    .memory_type_index(base.find_memory_type_index(
                        MemoryPropertyFlags::from_raw(memory_requirements.memory_type_bits),
                        settings.properties,
                    )?),
                None,
            )?
        };
        base.name_object(&memory, format!("{}Memory", settings.name.clone()))?;

        unsafe { base.device.bind_image_memory(image, memory, 0) }?;

        let view = unsafe {
            base.device.create_image_view(
                &ImageViewCreateInfo::builder()
                    .image(image)
                    .view_type(ImageViewType::TYPE_2D)
                    .format(settings.format)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .aspect_mask(settings.aspect_flags)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1)
                            .build(),
                    ),
                None,
            )
        }?;
        base.name_object(&view, format!("{}View", settings.name.clone()))?;

        Ok(Self {
            image,
            memory,
            view,
        })
    }

    pub unsafe fn drop(&self, device: &Device) {
        device.destroy_image_view(self.view, None);
        device.destroy_image(self.image, None);
        device.free_memory(self.memory, None);
    }
}
