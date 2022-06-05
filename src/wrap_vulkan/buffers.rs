use std::{marker::PhantomData, mem::size_of};

use anyhow::Result;
use ash::{
    vk::{
        Buffer, BufferCreateInfo, BufferUsageFlags, DeviceMemory, DeviceSize, MemoryAllocateInfo,
        MemoryMapFlags, MemoryPropertyFlags, SharingMode, WHOLE_SIZE,
    },
    Device,
};

use super::Context;

pub struct DeviceBuffer<T> {
    pub handle: Buffer,
    pub memory: DeviceMemory,
    pub len: usize,
    pub _phantom: PhantomData<T>, // to store the type that is stored
    device: Device,
}

impl<T> Drop for DeviceBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.handle, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

pub struct MappedDeviceBuffer<T> {
    buffer: DeviceBuffer<T>,
    mapped_ptr: *mut T,
}

impl<T> DeviceBuffer<T> {
    pub fn new(
        context: &Context,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
        len: usize,
        name: String,
    ) -> Result<Self> {
        let size = (len * size_of::<T>()) as DeviceSize;

        let handle = unsafe {
            context.device.create_buffer(
                &BufferCreateInfo::builder()
                    .size(size)
                    .usage(usage)
                    .sharing_mode(SharingMode::EXCLUSIVE),
                None,
            )
        }?;
        context.name_object(handle, format!("{}Handle", name))?;

        let memory = unsafe {
            context.device.allocate_memory(
                &MemoryAllocateInfo::builder()
                    .allocation_size(size)
                    .memory_type_index(
                        context.find_memory_type_index(
                            MemoryPropertyFlags::from_raw(
                                context
                                    .device
                                    .get_buffer_memory_requirements(handle)
                                    .memory_type_bits,
                            ),
                            properties,
                        )?,
                    ),
                None,
            )
        }?;
        context.name_object(memory, format!("{}Memory", name))?;

        unsafe { context.device.bind_buffer_memory(handle, memory, 0) }?;
        Ok(Self {
            handle,
            memory,
            len,
            _phantom: PhantomData,
            device: context.device.clone(),
        })
    }
}

impl<T> MappedDeviceBuffer<T> {
    pub fn new(
        context: &Context,
        usage: BufferUsageFlags,
        len: usize,
        name: String,
    ) -> Result<Self> {
        let buffer = DeviceBuffer::new(
            context,
            usage,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            len,
            name.clone(),
        )?;
        let mapped_ptr = unsafe {
            context
                .device
                .map_memory(buffer.memory, 0, WHOLE_SIZE, MemoryMapFlags::empty())
        }? as *mut T;

        Ok(Self { buffer, mapped_ptr })
    }

    pub fn handle(&self) -> Buffer {
        self.buffer.handle
    }

    pub fn write(&self, data: &[T]) {
        assert!(data.len() <= self.buffer.len);
        unsafe {
            self.mapped_ptr
                .copy_from_nonoverlapping(data.as_ptr(), data.len());
        }
    }

    pub fn size(&self) -> usize {
        self.buffer.len
    }
}
