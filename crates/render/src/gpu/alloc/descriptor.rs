use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context;
use ash::vk;

pub struct DescriptorHeap {
    mapped_ptr: *mut u8,
    device: ash::Device,
    device_memory: vk::DeviceMemory,
    buffer: vk::Buffer,

    gpu_base: vk::DeviceAddress,
    head: AtomicUsize,
    capacity: usize,
    alignment: usize,
    num_allocations: AtomicUsize,
}

impl DescriptorHeap {
    /// Extra padding applied as workaround to a `VK_ERROR_OUT_OF_DEVICE_MEMORY` issue with renderdoc
    const ALLOCATION_PADDING: u64 = 64 * 1024;

    pub fn new(
        device: ash::Device,
        memory_type_index: u32,
        capacity: u64,
        alignment: u64,
    ) -> anyhow::Result<Self> {
        let alignment = alignment.max(256) as usize;

        unsafe {
            let mut allocate_flags = vk::MemoryAllocateFlagsInfo::default()
                .flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);
            let allocate_info = vk::MemoryAllocateInfo::default()
                .push_next(&mut allocate_flags)
                .allocation_size(capacity + Self::ALLOCATION_PADDING)
                .memory_type_index(memory_type_index);
            let device_memory = device.allocate_memory(&allocate_info, None)?;
            let buffer = device.create_buffer(
                &vk::BufferCreateInfo::default().size(capacity).usage(
                    vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT
                        | vk::BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                ),
                None,
            )?;
            device.bind_buffer_memory(buffer, device_memory, 0)?;

            let gpu_base = device
                .get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(buffer));
            let mapped_ptr = device
                .map_memory(device_memory, 0, capacity, vk::MemoryMapFlags::empty())
                .context("failed to map descriptor buffer memory")?
                .cast::<u8>();

            mapped_ptr.write_bytes(0x00, alignment);

            Ok(Self {
                device,
                mapped_ptr,
                device_memory,
                buffer,
                gpu_base,
                head: AtomicUsize::new(alignment),
                capacity: capacity as usize,
                alignment,
                num_allocations: AtomicUsize::new(0),
            })
        }
    }

    pub fn reset(&self) {
        self.head.store(self.alignment, Ordering::Relaxed);
        self.num_allocations.store(0, Ordering::Relaxed);
    }

    pub fn alloc(&self, size: u64) -> anyhow::Result<usize> {
        let size = (size as usize).next_multiple_of(self.alignment);
        let start = self.head.fetch_add(size, Ordering::Relaxed);
        self.num_allocations.fetch_add(1, Ordering::Relaxed);

        if start + size > self.capacity {
            Err(anyhow::anyhow!("Descriptor ring out of memory"))
        } else {
            Ok(start)
        }
    }

    pub const fn ptr_at(&self, offset: usize) -> *mut u8 {
        unsafe { self.mapped_ptr.add(offset) }
    }

    pub const fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub const fn device_address(&self) -> vk::DeviceAddress {
        self.gpu_base
    }

    pub fn binding_info(&self) -> vk::DescriptorBufferBindingInfoEXT<'static> {
        vk::DescriptorBufferBindingInfoEXT::default()
            .address(self.gpu_base)
            .usage(
                vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT
                    | vk::BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT,
            )
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn used(&self) -> usize {
        self.head.load(Ordering::Relaxed)
    }

    pub fn num_allocations(&self) -> usize {
        self.num_allocations.load(Ordering::Relaxed)
    }
}

impl Drop for DescriptorHeap {
    fn drop(&mut self) {
        unsafe {
            self.device.unmap_memory(self.device_memory);
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.device_memory, None);
        }
    }
}
