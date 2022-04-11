use anyhow::{Error, Result};
use ash::vk::{BufferUsageFlags, DescriptorSet, DescriptorType, ShaderStageFlags};
use cgmath::Matrix4;
use crevice::std140::AsStd140;
use itertools::izip;
use vrv::wrap_vulkan::{
    buffers::MappedDeviceBuffer,
    descriptors::{DescriptorRelated, Usage},
    geometry::{Mesh, MeshBuffers},
    Base,
};

#[derive(AsStd140)]
pub struct UniformMatrices {
    pub model: Matrix4<f32>,
    pub view: Matrix4<f32>,
    pub proj: Matrix4<f32>,
}

pub struct PerFrame {
    pub matrix_buffer: MappedDeviceBuffer<UniformMatrices>,
    pub mesh_buffers: MeshBuffers,
    pub descriptor_set: DescriptorSet,
}

impl PerFrame {
    pub fn new_vec(base: &Base) -> Result<(Vec<Self>, DescriptorRelated)> {

        let debug_mesh = Mesh::load_gltf("examples/simple/untitled.glb")?;

        let image_count = base.get_image_count()?;

        let matrix_buffers: Vec<MappedDeviceBuffer<UniformMatrices>> = (0..image_count)
            .into_iter()
            .map(|index| {
                let matrix_buffer = MappedDeviceBuffer::new(
                    base,
                    BufferUsageFlags::UNIFORM_BUFFER,
                    1,
                    format!("WindowMatrices_{}", index),
                )?;
                matrix_buffer.write(&[UniformMatrices {
                    model: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ]
                    .into(),
                    view: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ]
                    .into(),
                    proj: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ]
                    .into(),
                }]);

                Ok(matrix_buffer)
            })
            .collect::<Result<_, Error>>()?;

        let mesh_buffers_s: Vec<MeshBuffers> = (0..image_count)
            .into_iter()
            .map(|index| {
                let mut mesh_buffers = MeshBuffers::new(
                    base,
                    debug_mesh.vertices.len(),
                    debug_mesh.indices.len(),
                    format!("WindowMeshBuffers_{}", index),
                )?;
                mesh_buffers.write(base, &debug_mesh)?;

                Ok(mesh_buffers)
            })
            .collect::<Result<_, Error>>()?;

        let (descriptor_related, descriptor_sets) = DescriptorRelated::new_with_sets(
            base,
            [(
                0,
                (DescriptorType::UNIFORM_BUFFER, ShaderStageFlags::VERTEX),
            )]
            .into(),
            &matrix_buffers
                .iter()
                .map(|buffer| [(0, Usage::Buffer(buffer.handle()))].into())
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
