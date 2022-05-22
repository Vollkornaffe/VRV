use anyhow::{Error, Result};
use ash::vk::{BufferUsageFlags, DescriptorSet, DescriptorType, ShaderStageFlags};
use cgmath::{Matrix4, SquareMatrix};
use crevice::std140::AsStd140;
use itertools::izip;
use vrv::wrap_vulkan::{
    buffers::MappedDeviceBuffer,
    descriptors::{DescriptorRelated, Usage},
    geometry::{Mesh, MeshBuffers},
    Context,
};

#[derive(AsStd140, Debug)]
pub struct UniformMatricesHMD {
    pub model: Matrix4<f32>,
    pub view_left: Matrix4<f32>,
    pub view_right: Matrix4<f32>,
    pub proj_left: Matrix4<f32>,
    pub proj_right: Matrix4<f32>,
}

#[derive(AsStd140, Debug)]
pub struct UniformMatricesWindow {
    pub model: Matrix4<f32>,
    pub view: Matrix4<f32>,
    pub proj: Matrix4<f32>,
}

#[derive(Debug)]
pub struct Buffer<UniformMatrices> {
    pub matrix_buffer: MappedDeviceBuffer<UniformMatrices>,
    pub mesh_buffers: MeshBuffers,
}

impl<UniformMatrices> Buffer<UniformMatrices> {
    pub fn new(context: &Context, name: String) -> Result<Self> {
        let matrix_buffer = MappedDeviceBuffer::new(
            context,
            BufferUsageFlags::UNIFORM_BUFFER,
            1,
            format!("{}Matrices", name),
        )?;

        let debug_mesh = Mesh::load_gltf("examples/simple/untitled.glb")?;
        let mut mesh_buffers = MeshBuffers::new(
            context,
            debug_mesh.vertices.len(),
            debug_mesh.indices.len(),
            format!("{}MeshBuffers", name),
        )?;
        mesh_buffers.write(context, &debug_mesh)?;

        Ok(Self {
            matrix_buffer,
            mesh_buffers,
        })
    }
}
