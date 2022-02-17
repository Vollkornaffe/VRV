use std::mem::size_of;

use ash::vk::{
    Format, VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};
use memoffset::offset_of;

#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub col: [f32; 3],
}

impl Vertex {
    pub fn debug_triangle() -> Vec<Self> {
        vec![
            Self {
                pos: [0.0, -0.5, 0.0],
                col: [1.0, 0.0, 0.0],
            },
            Self {
                pos: [0.5, 0.5, 0.0],
                col: [0.0, 1.0, 0.0],
            },
            Self {
                pos: [-0.5, 0.5, 0.0],
                col: [0.0, 0.0, 1.0],
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
