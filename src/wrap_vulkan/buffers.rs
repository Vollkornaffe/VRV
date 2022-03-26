use std::{marker::PhantomData, mem::size_of};

use anyhow::Result;
use ash::vk::{
    Buffer, BufferCreateInfo, BufferUsageFlags, DeviceMemory, DeviceSize, MemoryAllocateInfo,
    MemoryMapFlags, MemoryPropertyFlags, SharingMode, WHOLE_SIZE,
};

use super::Base;

pub struct DeviceBuffer<T> {
    pub handle: Buffer,
    pub memory: DeviceMemory,
    pub len: usize,
    pub _phantom: PhantomData<T>, // to store the type that is stored
}

pub struct MappedDeviceBuffer<T> {
    buffer: DeviceBuffer<T>,
    mapped_ptr: *mut T,
}

impl<T> DeviceBuffer<T> {
    pub fn new(
        base: &Base,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
        len: usize,
        name: String,
    ) -> Result<Self> {
        let size = (len * size_of::<T>()) as DeviceSize;

        let handle = unsafe {
            base.device.create_buffer(
                &BufferCreateInfo::builder()
                    .size(size)
                    .usage(usage)
                    .sharing_mode(SharingMode::EXCLUSIVE),
                None,
            )
        }?;
        base.name_object(handle, format!("{}Handle", name))?;

        let memory = unsafe {
            base.device.allocate_memory(
                &MemoryAllocateInfo::builder()
                    .allocation_size(size)
                    .memory_type_index(
                        base.find_memory_type_index(
                            MemoryPropertyFlags::from_raw(
                                base.device
                                    .get_buffer_memory_requirements(handle)
                                    .memory_type_bits,
                            ),
                            properties,
                        )?,
                    ),
                None,
            )
        }?;
        base.name_object(memory, format!("{}Memory", name))?;

        unsafe { base.device.bind_buffer_memory(handle, memory, 0) }?;
        Ok(Self {
            handle,
            memory,
            len,
            _phantom: PhantomData,
        })
    }

    pub unsafe fn destroy(&self, base: &Base) {
        base.device.destroy_buffer(self.handle, None);
        base.device.free_memory(self.memory, None);
    }
}

impl<T> MappedDeviceBuffer<T> {
    pub fn new(base: &Base, usage: BufferUsageFlags, len: usize, name: String) -> Result<Self> {
        let buffer = DeviceBuffer::new(
            base,
            usage,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            len,
            name.clone(),
        )?;
        let mapped_ptr = unsafe {
            base.device
                .map_memory(buffer.memory, 0, WHOLE_SIZE, MemoryMapFlags::empty())
        }? as *mut T;

        Ok(Self { buffer, mapped_ptr })
    }

    pub fn handle(&self) -> Buffer {
        self.buffer.handle
    }

    pub fn write(&mut self, data: &[T]) {
        assert!(data.len() <= self.buffer.len);
        unsafe {
            self.mapped_ptr
                .copy_from_nonoverlapping(data.as_ptr(), data.len());
        }
    }

    pub fn size(&self) -> usize {
        self.buffer.len
    }

    pub unsafe fn destroy(&self, base: &Base) {
        self.buffer.destroy(base);
    }
}
