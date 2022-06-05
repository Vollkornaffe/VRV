use anyhow::Result;
use gltf::import;
use itertools::izip;
use std::{mem::size_of, path::Path};

use ash::vk::{
    Buffer, BufferUsageFlags, Format, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate,
};
use memoffset::offset_of;

use super::{buffers::MappedDeviceBuffer, Context};

#[derive(Debug)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub col: [f32; 3],
}

impl Vertex {
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
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn debug_triangle() -> Self {
        let vertices = vec![
            Vertex {
                pos: [0.0, -0.5, 0.0].into(),
                col: [1.0, 0.0, 0.0].into(),
            },
            Vertex {
                pos: [0.5, 0.5, 0.0].into(),
                col: [0.0, 1.0, 0.0].into(),
            },
            Vertex {
                pos: [-0.5, 0.5, 0.0].into(),
                col: [0.0, 0.0, 1.0].into(),
            },
        ];
        let indices = vec![0, 1, 2];
        Self { vertices, indices }
    }

    pub fn load_gltf<P: AsRef<Path>>(filename: P) -> Result<Self> {
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
    pub vertex: MappedDeviceBuffer<Vertex>,
    pub index: MappedDeviceBuffer<u32>,
    pub name: String,
}

impl MeshBuffers {
    pub fn new(context: &Context, vertices: usize, indices: usize, name: String) -> Result<Self> {
        let vertex = MappedDeviceBuffer::new(
            context,
            BufferUsageFlags::VERTEX_BUFFER,
            vertices,
            format!("{}Vertex", name),
        )?;
        let index = MappedDeviceBuffer::new(
            context,
            BufferUsageFlags::INDEX_BUFFER,
            indices,
            format!("{}Index", name),
        )?;

        Ok(Self {
            vertex,
            index,
            name,
        })
    }

    pub fn resize_vertex(&mut self, context: &Context, new_size: usize) -> Result<()> {
        if self.vertex.size() == new_size {
            return Ok(());
        }

        self.vertex = MappedDeviceBuffer::new(
            context,
            BufferUsageFlags::VERTEX_BUFFER,
            new_size,
            format!("{}Vertex", self.name),
        )?;

        Ok(())
    }

    pub fn resize_index(&mut self, context: &Context, new_size: usize) -> Result<()> {
        if self.index.size() == new_size {
            return Ok(());
        }

        self.index = MappedDeviceBuffer::new(
            context,
            BufferUsageFlags::INDEX_BUFFER,
            new_size,
            format!("{}Index", self.name),
        )?;

        Ok(())
    }

    pub fn write(&mut self, context: &Context, mesh: &Mesh) -> Result<()> {
        if self.vertex.size() < mesh.vertices.len() {
            self.resize_vertex(context, mesh.vertices.len())?;
        }

        if self.index.size() < mesh.indices.len() {
            self.resize_index(context, mesh.indices.len())?;
        }

        self.vertex.write(&mesh.vertices);
        self.index.write(&mesh.indices);

        Ok(())
    }

    pub fn num_vertices(&self) -> usize {
        self.vertex.size()
    }

    pub fn num_indices(&self) -> usize {
        self.index.size()
    }

    pub fn vertex_buffer(&self) -> Buffer {
        self.vertex.handle()
    }

    pub fn index_buffer(&self) -> Buffer {
        self.index.handle()
    }
}
