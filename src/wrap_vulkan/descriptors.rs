use std::collections::HashMap;

use anyhow::{Error, Result};
use ash::vk::{
    Buffer, DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool, DescriptorPoolCreateInfo,
    DescriptorPoolSize, DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, ImageLayout,
    ImageView, Sampler, ShaderStageFlags, WriteDescriptorSet, WHOLE_SIZE,
};

use super::Base;

pub struct DescriptorSets {
    pub layout: DescriptorSetLayout,
    pub pool: DescriptorPool,
    pub sets: Vec<DescriptorSet>,
}

#[derive(Clone, Copy)]
pub enum Usage {
    Buffer(Buffer),
    ImageSampler(ImageLayout, ImageView, Sampler),
}

impl DescriptorSets {
    pub fn new(
        base: &Base,
        setup: HashMap<u32, (DescriptorType, ShaderStageFlags)>,
        usages: &[HashMap<u32, Usage>],
        name: String,
    ) -> Result<Self> {
        let layout = unsafe {
            base.device.create_descriptor_set_layout(
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
        base.name_object(layout, format!("{}Layout", name))?;

        let num_sets = usages.len() as u32;
        let pool = unsafe {
            base.device.create_descriptor_pool(
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
        base.name_object(pool, format!("{}Pool", name))?;

        let sets: Vec<DescriptorSet> = usages
            .iter()
            .enumerate()
            .map(|(i, usage_map)| {
                let set = unsafe {
                    base.device.allocate_descriptor_sets(
                        &DescriptorSetAllocateInfo::builder()
                            .descriptor_pool(pool)
                            .set_layouts(&[layout]),
                    )
                }?[0];
                base.name_object(set, format!("{}Set_{}", name, i))?;

                let base_write = |set, binding| {
                    WriteDescriptorSet::builder()
                        .dst_set(set)
                        .dst_binding(binding)
                        .dst_array_element(0)
                        .descriptor_type(
                            setup
                                .get(&binding)
                                .expect("Invalid binding for descriptor")
                                .0,
                        )
                };
                unsafe {
                    base.device.update_descriptor_sets(
                        &usage_map
                            .iter()
                            // TODO I don't really believe this works
                            .map(|(&binding, &usage)| match usage {
                                Usage::Buffer(buffer) => base_write(set, binding)
                                    .buffer_info(&[DescriptorBufferInfo::builder()
                                        .buffer(buffer)
                                        .offset(0)
                                        .range(WHOLE_SIZE)
                                        .build()])
                                    .build(),
                                Usage::ImageSampler(image_layout, image_view, sampler) => {
                                    base_write(set, binding)
                                        .image_info(&[DescriptorImageInfo::builder()
                                            .image_layout(image_layout)
                                            .image_view(image_view)
                                            .sampler(sampler)
                                            .build()])
                                        .build()
                                }
                            })
                            .collect::<Vec<_>>(),
                        &[], // no copies
                    )
                }

                Ok(set)
            })
            .collect::<Result<_, Error>>()?;

        Ok(Self { layout, pool, sets })
    }

    pub unsafe fn destroy(&self, base: &Base) {
        base.device.destroy_descriptor_pool(self.pool, None);
        base.device.destroy_descriptor_set_layout(self.layout, None);
    }
}
