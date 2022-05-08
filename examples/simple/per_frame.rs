use anyhow::{Error, Result};
use ash::vk::{
    BufferUsageFlags, DescriptorSet, DescriptorType, ImageLayout, Sampler, ShaderStageFlags,
};
use cgmath::{Matrix4, SquareMatrix};
use crevice::std140::AsStd140;
use itertools::izip;
use vrv::wrap_vulkan::{
    buffers::MappedDeviceBuffer,
    descriptors::{DescriptorRelated, Usage},
    geometry::{Mesh, MeshBuffers},
    Context, DeviceImage,
};

#[derive(AsStd140)]
pub struct UniformMatricesHMD {
    pub model: Matrix4<f32>,
    pub view_left: Matrix4<f32>,
    pub view_right: Matrix4<f32>,
    pub proj_left: Matrix4<f32>,
    pub proj_right: Matrix4<f32>,
}

#[derive(AsStd140)]
pub struct UniformMatricesWindow {
    pub model: Matrix4<f32>,
    pub view: Matrix4<f32>,
    pub proj: Matrix4<f32>,
}

pub struct PerFrameHMD {
    pub matrix_buffer: MappedDeviceBuffer<UniformMatricesHMD>,
    pub mesh_buffers: MeshBuffers,
    pub descriptor_set: DescriptorSet,
}

pub struct PerFrameWindow {
    pub matrix_buffer: MappedDeviceBuffer<UniformMatricesWindow>,
    pub mesh_buffers: MeshBuffers,
    pub descriptor_set: DescriptorSet,
}

impl PerFrameWindow {
    pub fn new_vec(
        context: &Context,
        debug_mesh: &Mesh,
        debug_texture: &DeviceImage,
        font_texture: &DeviceImage,
        sampler: Sampler,
        image_count: u32,
    ) -> Result<(Vec<Self>, DescriptorRelated)> {
        let matrix_buffers: Vec<MappedDeviceBuffer<UniformMatricesWindow>> = (0..image_count)
            .into_iter()
            .map(|index| {
                let matrix_buffer = MappedDeviceBuffer::new(
                    context,
                    BufferUsageFlags::UNIFORM_BUFFER,
                    1,
                    format!("WindowMatrices_{}", index),
                )?;
                matrix_buffer.write(&[UniformMatricesWindow {
                    model: Matrix4::identity(),
                    view: Matrix4::identity(),
                    proj: Matrix4::identity(),
                }]);

                Ok(matrix_buffer)
            })
            .collect::<Result<_, Error>>()?;

        let mesh_buffers_s: Vec<MeshBuffers> = (0..image_count)
            .into_iter()
            .map(|index| {
                let mut mesh_buffers = MeshBuffers::new(
                    context,
                    debug_mesh.vertices.len(),
                    debug_mesh.indices.len(),
                    format!("WindowMeshBuffers_{}", index),
                )?;
                mesh_buffers.write(context, &debug_mesh)?;

                Ok(mesh_buffers)
            })
            .collect::<Result<_, Error>>()?;

        let (descriptor_related, descriptor_sets) = DescriptorRelated::new_with_sets(
            context,
            [
                (
                    0,
                    (DescriptorType::UNIFORM_BUFFER, ShaderStageFlags::VERTEX),
                ),
                (
                    1,
                    (
                        DescriptorType::COMBINED_IMAGE_SAMPLER,
                        ShaderStageFlags::FRAGMENT,
                    ),
                ),
                (
                    2,
                    (
                        DescriptorType::COMBINED_IMAGE_SAMPLER,
                        ShaderStageFlags::FRAGMENT,
                    ),
                ),
            ]
            .into(),
            &matrix_buffers
                .iter()
                .map(|buffer| {
                    [
                        (0, Usage::Buffer(buffer.handle())),
                        (
                            1,
                            Usage::ImageSampler(
                                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                                debug_texture.view,
                                sampler,
                            ),
                        ),
                        (
                            2,
                            Usage::ImageSampler(
                                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                                font_texture.view,
                                sampler,
                            ),
                        ),
                    ]
                    .into()
                })
                .collect::<Vec<_>>(),
            "WindowDescriptorSets".to_string(),
        )?;
        Ok((
            izip!(
                matrix_buffers.into_iter(),
                mesh_buffers_s.into_iter(),
                descriptor_sets.into_iter()
            )
            .map(|(matrix_buffer, mesh_buffers, descriptor_set)| Self {
                matrix_buffer,
                mesh_buffers,
                descriptor_set,
            })
            .collect(),
            descriptor_related,
        ))
    }
}

impl PerFrameHMD {
    pub fn new_vec(
        context: &Context,
        debug_mesh: &Mesh,
        debug_texture: &DeviceImage,
        font_texture: &DeviceImage,
        sampler: Sampler,
        image_count: u32,
    ) -> Result<(Vec<Self>, DescriptorRelated)> {
        let matrix_buffers: Vec<MappedDeviceBuffer<UniformMatricesHMD>> = (0..image_count)
            .into_iter()
            .map(|index| {
                let matrix_buffer = MappedDeviceBuffer::new(
                    context,
                    BufferUsageFlags::UNIFORM_BUFFER,
                    1,
                    format!("HMDMatrices_{}", index),
                )?;
                matrix_buffer.write(&[UniformMatricesHMD {
                    model: Matrix4::identity(),
                    view_left: Matrix4::identity(),
                    view_right: Matrix4::identity(),
                    proj_left: Matrix4::identity(),
                    proj_right: Matrix4::identity(),
                }]);

                Ok(matrix_buffer)
            })
            .collect::<Result<_, Error>>()?;

        let mesh_buffers_s: Vec<MeshBuffers> = (0..image_count)
            .into_iter()
            .map(|index| {
                let mut mesh_buffers = MeshBuffers::new(
                    context,
                    debug_mesh.vertices.len(),
                    debug_mesh.indices.len(),
                    format!("HMDMeshBuffers_{}", index),
                )?;
                mesh_buffers.write(context, &debug_mesh)?;

                Ok(mesh_buffers)
            })
            .collect::<Result<_, Error>>()?;

        let (descriptor_related, descriptor_sets) = DescriptorRelated::new_with_sets(
            context,
            [
                (
                    0,
                    (DescriptorType::UNIFORM_BUFFER, ShaderStageFlags::VERTEX),
                ),
                (
                    1,
                    (
                        DescriptorType::COMBINED_IMAGE_SAMPLER,
                        ShaderStageFlags::FRAGMENT,
                    ),
                ),
                (
                    2,
                    (
                        DescriptorType::COMBINED_IMAGE_SAMPLER,
                        ShaderStageFlags::FRAGMENT,
                    ),
                ),
            ]
            .into(),
            &matrix_buffers
                .iter()
                .map(|buffer| {
                    [
                        (0, Usage::Buffer(buffer.handle())),
                        (
                            1,
                            Usage::ImageSampler(
                                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                                debug_texture.view,
                                sampler,
                            ),
                        ),
                        (
                            2,
                            Usage::ImageSampler(
                                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                                font_texture.view,
                                sampler,
                            ),
                        ),
                    ]
                    .into()
                })
                .collect::<Vec<_>>(),
            "HMDDescriptorSets".to_string(),
        )?;
        Ok((
            izip!(
                matrix_buffers.into_iter(),
                mesh_buffers_s.into_iter(),
                descriptor_sets.into_iter()
            )
            .map(|(matrix_buffer, mesh_buffers, descriptor_set)| Self {
                matrix_buffer,
                mesh_buffers,
                descriptor_set,
            })
            .collect(),
            descriptor_related,
        ))
    }
}
