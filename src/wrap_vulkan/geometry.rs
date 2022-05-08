use anyhow::Result;
use gltf::import;
use itertools::izip;
use std::{mem::size_of, path::Path};

use ash::vk::{
    Buffer, BufferUsageFlags, Extent2D, Format, FormatFeatureFlags, ImageAspectFlags, ImageTiling,
    VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};
use memoffset::offset_of;

use crate::context::texture::create_texture;

use super::{buffers::MappedDeviceBuffer, Context, DeviceImage};

#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nor: [f32; 3],
    pub uv: [f32; 2],
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
                .offset(offset_of!(Self, nor) as u32)
                .build(),
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(2)
                .format(Format::R32G32_SFLOAT)
                .offset(offset_of!(Self, uv) as u32)
                .build(),
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(3)
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
                nor: [0.0, 0.0, 1.0].into(),
                uv: [0.0, -0.5].into(),
                col: [1.0, 0.0, 0.0].into(),
            },
            Vertex {
                pos: [0.5, 0.5, 0.0].into(),
                nor: [0.0, 0.0, 1.0].into(),
                uv: [0.5, 0.5].into(),
                col: [0.0, 1.0, 0.0].into(),
            },
            Vertex {
                pos: [-0.5, 0.5, 0.0].into(),
                nor: [0.0, 0.0, 1.0].into(),
                uv: [-0.5, 0.5].into(),
                col: [0.0, 0.0, 1.0].into(),
            },
        ];
        let indices = vec![0, 1, 2];
        Self { vertices, indices }
    }

    pub fn load_gltf<P: AsRef<Path>>(
        context: &Context,
        filename: P,
    ) -> Result<(Self, DeviceImage)> {
        let (gltf, buffers, images) = import(filename)?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let map_format = |format| match format {
            gltf::image::Format::R8 => Format::R8_SRGB,
            gltf::image::Format::R8G8 => Format::R8G8_SRGB,
            gltf::image::Format::R8G8B8 => Format::R8G8B8_SRGB,
            gltf::image::Format::R8G8B8A8 => Format::R8G8B8A8_SRGB,
            gltf::image::Format::B8G8R8 => Format::B8G8R8_SRGB,
            gltf::image::Format::B8G8R8A8 => Format::B8G8R8A8_SRGB,
            gltf::image::Format::R16 => Format::R16_SFLOAT,
            gltf::image::Format::R16G16 => Format::R16G16_SFLOAT,
            gltf::image::Format::R16G16B16 => Format::R16G16B16_SFLOAT,
            gltf::image::Format::R16G16B16A16 => Format::R16G16B16A16_SFLOAT,
        };

        let image = images
            .first()
            .expect("No image that can be used as texture");

        log::warn!(
            "Gltf texture has format {:?}, in vulkan {:?}",
            image.format,
            map_format(image.format)
        );

        let texture = create_texture(
            context,
            Extent2D {
                width: image.width,
                height: image.height,
            },
            image.pixels.as_slice(),
            Format::R8G8B8A8_UNORM,
            ImageAspectFlags::COLOR,
            "GltfTexture".to_string(), // TODO
        )?;

        for mesh in gltf.meshes() {
            log::info!("Reading mesh: {}", mesh.name().or(Some("NO NAME")).unwrap());

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                indices.extend(
                    reader
                        .read_indices()
                        .expect("didn't find indices")
                        .into_u32()
                        .map(|i| i + vertices.len() as u32),
                );

                vertices.extend(
                    izip!(
                        reader.read_positions().expect("didn't find positions"),
                        reader.read_normals().expect("didn't find normals"),
                        reader
                            .read_tex_coords(0)
                            .expect("didn't find tex coords")
                            .into_f32(),
                        reader
                            .read_colors(0)
                            .expect("didn't find colors")
                            .into_rgb_f32(), // TODO what is the color set?
                    )
                    .map(|(pos, nor, uv, col)| Vertex {
                        pos: pos.into(),
                        nor: nor.into(),
                        uv: uv.into(),
                        col: col.into(),
                    }),
                );
            }
        }

        Ok((Self { vertices, indices }, texture))
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

        unsafe { self.vertex.destroy(context) };

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

        unsafe { self.index.destroy(context) };

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

    pub unsafe fn destroy(&self, context: &Context) {
        self.vertex.destroy(context);
        self.index.destroy(context);
    }
}
