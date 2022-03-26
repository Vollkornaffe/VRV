use anyhow::{Error, Result};
use gltf::import;
use itertools::izip;
use std::{fmt::format, mem::size_of, path::Path};

use ash::vk::{
    BufferUsageFlags, Format, MemoryPropertyFlags, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate,
};
use memoffset::offset_of;

use super::{buffers::MappedDeviceBuffer, Base};

/* Going to use crevice to handle this stuff, also not needed for vertex buffers just yet
#[repr(C, align(16))]
pub struct Align16<T: Copy>(pub T);
impl<T: Copy> From<T> for Align16<T> {
    fn from(t: T) -> Self {
        Self(t)
    }
}
*/

#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub col: [f32; 3],
}

impl Vertex {
    pub fn debug_triangle() -> Vec<Self> {
        vec![
            Self {
                pos: [0.0, -0.5, 0.0].into(),
                col: [1.0, 0.0, 0.0].into(),
            },
            Self {
                pos: [0.5, 0.5, 0.0].into(),
                col: [0.0, 1.0, 0.0].into(),
            },
            Self {
                pos: [-0.5, 0.5, 0.0].into(),
                col: [0.0, 0.0, 1.0].into(),
            },
        ]
    }

    pub fn get_binding_description() -> Vec<VertexInputBindingDescription> {
        vec![VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Self>() as u32)
            .input_rate(VertexInputRate::VERTEX)
            .build()]
    }

    pub fn get_attribute_description() -> Vec<VertexInputAttributeDescription> {
        vec![
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(Format::R32G32B32_SFLOAT)
                .offset(offset_of!(Self, pos) as u32)
                .build(),
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(Format::R32G32B32_SFLOAT)
                .offset(offset_of!(Self, col) as u32)
                .build(),
        ]
    }
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn load_gltf<P: AsRef<Path>>(base: &Base, filename: P) -> Result<Self> {
        let (gltf, buffers, _) = import(filename)?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in gltf.meshes() {
            log::debug!("Reading mesh: {}", mesh.name().or(Some("NO NAME")).unwrap());

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                indices.extend(
                    reader
                        .read_indices()
                        .expect("didn't find indices")
                        .into_u32()
                        .map(|i| i + vertices.len() as u32),
                );

                if reader.read_colors(0).is_some() {
                    vertices.extend(
                        izip!(
                            reader.read_positions().expect("didn't find positions"),
                            reader.read_normals().expect("didn't find normals"),
                            reader
                                .read_colors(0)
                                .expect("didn't find colors")
                                .into_rgb_f32(), // TODO what is the color set?
                        )
                        .map(|(p, _n, c)| Vertex {
                            // TODO use normal
                            pos: p.into(),
                            col: c.into(),
                        }),
                    );
                } else {
                    log::warn!("Didn't find no colors");
                    vertices.extend(
                        izip!(
                            reader.read_positions().expect("didn't find positions"),
                            reader.read_normals().expect("didn't find normals"),
                        )
                        .map(|(p, _n)| Vertex {
                            // TODO use normal
                            pos: p.into(),
                            col: [0.1, 0.2, 0.8], // blue-ish
                        }),
                    );
                }
            }
        }

        Ok(Self { vertices, indices })
    }
}

pub struct MeshBuffers {
    vertex: MappedDeviceBuffer<Vertex>,
    index: MappedDeviceBuffer<u32>,
}

impl MeshBuffers {
    pub fn new(base: &Base, vertices: usize, indices: usize, name: String) -> Result<Self> {
        let vertex = MappedDeviceBuffer::new(
            base,
            BufferUsageFlags::VERTEX_BUFFER,
            vertices,
            format!("{}Vertex", name),
        )?;
        let index = MappedDeviceBuffer::new(
            base,
            BufferUsageFlags::INDEX_BUFFER,
            indices,
            format!("{}Index", name),
        )?;

        Ok(Self { vertex, index })
    }
}

pub struct MappedMesh {
    cpu: Mesh,
    gpu: MeshBuffers,
}

impl MappedMesh {
    pub fn new(base: &Base, mesh: Mesh, name: String) -> Result<Self> {
        let cpu = mesh;
        let mut gpu = MeshBuffers::new(base, cpu.vertices.len(), cpu.indices.len(), name)?;

        gpu.vertex.write(&cpu.vertices);
        gpu.index.write(&cpu.indices);

        Ok(Self { cpu, gpu })
    }
}
