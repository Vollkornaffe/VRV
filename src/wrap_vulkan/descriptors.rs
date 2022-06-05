use std::collections::HashMap;

use anyhow::{Error, Result};
use ash::{
    vk::{
        Buffer, DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool,
        DescriptorPoolCreateInfo, DescriptorPoolSize, DescriptorSet, DescriptorSetAllocateInfo,
        DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
        DescriptorType, ImageLayout, ImageView, Sampler, ShaderStageFlags, WriteDescriptorSet,
        WHOLE_SIZE,
    },
    Device,
};

use super::Context;

pub struct DescriptorRelated {
    pub layout: DescriptorSetLayout,
    pool: DescriptorPool,
    device: Device,
}

impl Drop for DescriptorRelated {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_descriptor_pool(self.pool, None);
            self.device.destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

#[derive(Clone, Copy)]
pub enum Usage {
    Buffer(Buffer),
    ImageSampler(ImageLayout, ImageView, Sampler),
}

impl DescriptorRelated {
    pub fn new_with_sets(
        context: &Context,
        setup: HashMap<u32, (DescriptorType, ShaderStageFlags)>,
        usages: &[HashMap<u32, Usage>],
        name: String,
    ) -> Result<(Self, Vec<DescriptorSet>)> {
        let layout = unsafe {
            context.device.create_descriptor_set_layout(
                &DescriptorSetLayoutCreateInfo::builder().bindings(
                    &setup
                        .iter()
                        .map(|(&binding, &(ty, stage_flags))| {
                            DescriptorSetLayoutBinding::builder()
                                .binding(binding)
                                .descriptor_type(ty)
                                .descriptor_count(1)
                                .stage_flags(stage_flags)
                                .build()
                        })
                        .collect::<Vec<_>>(),
                ),
                None,
            )
        }?;
        context.name_object(layout, format!("{}Layout", name))?;

        let num_sets = usages.len() as u32;
        let pool = unsafe {
            context.device.create_descriptor_pool(
                &DescriptorPoolCreateInfo::builder()
                    .pool_sizes(
                        &[
                            DescriptorType::SAMPLER,
                            DescriptorType::COMBINED_IMAGE_SAMPLER,
                            DescriptorType::SAMPLED_IMAGE,
                            DescriptorType::STORAGE_IMAGE,
                            DescriptorType::UNIFORM_TEXEL_BUFFER,
                            DescriptorType::STORAGE_TEXEL_BUFFER,
                            DescriptorType::UNIFORM_BUFFER,
                            DescriptorType::STORAGE_BUFFER,
                            DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                            DescriptorType::STORAGE_BUFFER_DYNAMIC,
                            DescriptorType::INPUT_ATTACHMENT,
                        ]
                        .iter()
                        .filter_map(|&ty| {
                            let match_count = setup
                                .iter()
                                .filter(|&(_, &(used_ty, _))| ty == used_ty)
                                .count() as u32;
                            if match_count > 0 {
                                Some(
                                    DescriptorPoolSize::builder()
                                        .ty(ty)
                                        .descriptor_count(match_count * num_sets)
                                        .build(),
                                )
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                    )
                    .max_sets(num_sets),
                None,
            )
        }?;
        context.name_object(pool, format!("{}Pool", name))?;

        let sets: Vec<DescriptorSet> = usages
            .iter()
            .enumerate()
            .map(|(i, usage_map)| {
                let set = unsafe {
                    context.device.allocate_descriptor_sets(
                        &DescriptorSetAllocateInfo::builder()
                            .descriptor_pool(pool)
                            .set_layouts(&[layout]),
                    )
                }?[0];
                context.name_object(set, format!("{}Set_{}", name, i))?;

                struct Info {
                    binding: u32,
                    buffer_infos: Vec<DescriptorBufferInfo>,
                    image_infos: Vec<DescriptorImageInfo>,
                }

                let infos: Vec<Info> = usage_map
                    .iter()
                    .map(|(&binding, &usage)| match usage {
                        Usage::Buffer(buffer) => Info {
                            binding,
                            buffer_infos: vec![DescriptorBufferInfo::builder()
                                .buffer(buffer)
                                .offset(0)
                                .range(WHOLE_SIZE)
                                .build()],
                            image_infos: vec![],
                        },
                        Usage::ImageSampler(image_layout, image_view, sampler) => Info {
                            binding,
                            buffer_infos: vec![],
                            image_infos: vec![DescriptorImageInfo::builder()
                                .image_layout(image_layout)
                                .image_view(image_view)
                                .sampler(sampler)
                                .build()],
                        },
                    })
                    .collect();

                unsafe {
                    context.device.update_descriptor_sets(
                        &infos
                            .iter()
                            .map(|info| {
                                let incomplete = WriteDescriptorSet::builder()
                                    .dst_set(set)
                                    .dst_binding(info.binding)
                                    .dst_array_element(0)
                                    .descriptor_type(
                                        setup
                                            .get(&info.binding)
                                            .expect("Invalid binding for descriptor")
                                            .0,
                                    );
                                if !info.buffer_infos.is_empty() {
                                    return incomplete
                                        .buffer_info(info.buffer_infos.as_slice())
                                        .build();
                                }
                                if !info.image_infos.is_empty() {
                                    return incomplete
                                        .image_info(info.image_infos.as_slice())
                                        .build();
                                }
                                panic!("No buffer infos and no image infos");
                            })
                            .collect::<Vec<_>>(),
                        &[], // no copies
                    )
                }

                Ok(set)
            })
            .collect::<Result<_, Error>>()?;

        Ok((
            Self {
                layout,
                pool,
                device: context.device.clone(),
            },
            sets,
        ))
    }
}
