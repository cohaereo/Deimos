use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context;
use ash::vk;

/// Lock-free thread-safe ring buffer for uploading data to the GPU.
pub struct UploadHeap {
    mapped_ptr: *mut u8,
    device: ash::Device,
    device_memory: vk::DeviceMemory,
    buffer: vk::Buffer,

    gpu_base: vk::DeviceAddress,
    head: AtomicUsize,
    capacity: usize,
    num_allocations: AtomicUsize,
}

impl UploadHeap {
    const BLOCK_ALIGNMENT: usize = 256;

    pub fn new(device: ash::Device, memory_type_index: u32, capacity: u64) -> anyhow::Result<Self> {
        unsafe {
            let allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(capacity)
                .memory_type_index(memory_type_index);
            let device_memory = device.allocate_memory(&allocate_info, None)?;
            let buffer = device.create_buffer(
                &vk::BufferCreateInfo::default().size(capacity).usage(
                    vk::BufferUsageFlags::TRANSFER_SRC
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::UNIFORM_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                ),
                None,
            )?;
            device.bind_buffer_memory(buffer, device_memory, 0)?;

            let gpu_base = device
                .get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(buffer));
            let mapped_ptr = device
                .map_memory(device_memory, 0, capacity, vk::MemoryMapFlags::empty())
                .context("failed to map host-visible memory")?
                .cast::<u8>();

            mapped_ptr.write_bytes(0xCC, Self::BLOCK_ALIGNMENT);

            Ok(Self {
                device,
                mapped_ptr,
                device_memory,
                buffer,
                gpu_base,
                // First block is reserved for null pointer
                head: AtomicUsize::new(Self::BLOCK_ALIGNMENT),
                capacity: capacity as usize,
                num_allocations: AtomicUsize::new(0),
            })
        }
    }

    pub fn reset(&self) {
        self.head.store(Self::BLOCK_ALIGNMENT, Ordering::Relaxed);
        self.num_allocations.store(0, Ordering::Relaxed);
    }

    pub fn alloc_slice(&self, size: usize) -> anyhow::Result<RingSlice> {
        let size_aligned = size.next_multiple_of(Self::BLOCK_ALIGNMENT);
        let start = self.head.fetch_add(size_aligned, Ordering::Relaxed);
        self.num_allocations.fetch_add(1, Ordering::Relaxed);

        debug_assert!(
            start.is_multiple_of(Self::BLOCK_ALIGNMENT),
            "Misaligned ring buffer"
        );
        if start + size_aligned > self.capacity {
            Err(anyhow::anyhow!("Out of memory"))
        } else {
            let slice = RingSlice {
                ptr: unsafe { self.mapped_ptr.add(start) },
                len: size,
                capacity: size_aligned,
                gpu_va: self.gpu_base + start as u64,
            };

            unsafe {
                slice.ptr.write_bytes(0x00, slice.capacity());
            }

            Ok(slice)
        }
    }

    pub fn alloc<T: Sized>(&self) -> anyhow::Result<TypedRingAllocation<T>> {
        let slice = self.alloc_slice(std::mem::size_of::<T>())?;
        Ok(TypedRingAllocation {
            ptr: slice.ptr.cast::<T>(),
            gpu_va: slice.gpu_va,
        })
    }

    pub fn upload_bytes(&self, data: &[u8]) -> anyhow::Result<vk::DeviceAddress> {
        let slice = self.alloc_slice(data.len())?;
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), slice.ptr, data.len());
        }
        Ok(slice.gpu_va)
    }

    pub const fn null(&self) -> vk::DeviceAddress {
        self.gpu_base
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

impl Drop for UploadHeap {
    fn drop(&mut self) {
        unsafe {
            self.device.unmap_memory(self.device_memory);
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.device_memory, None);
        }
    }
}

pub struct RingSlice {
    ptr: *mut u8,
    len: usize,
    capacity: usize,

    gpu_va: vk::DeviceAddress,
}

impl RingSlice {
    pub const fn ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Returns the total capacity of the ring slice.
    ///
    /// This is the allocated length aligned up to 256 bytes
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the length of the ring slice.
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub const fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    pub const fn virtual_address(&self) -> vk::DeviceAddress {
        self.gpu_va
    }
}

pub struct TypedRingAllocation<T: Sized> {
    ptr: *mut T,
    gpu_va: vk::DeviceAddress,
}

impl<T: Sized> TypedRingAllocation<T> {
    pub const fn ptr(&self) -> *mut T {
        self.ptr
    }

    pub const fn write(&self, data: &T) {
        unsafe {
            self.ptr().copy_from(data as *const T, 1);
        }
    }

    pub const fn virtual_address(&self) -> vk::DeviceAddress {
        self.gpu_va
    }
}
