pub mod buffer;
pub mod descriptor;
mod guard;
pub mod image;
pub mod staging;
pub mod upload;

use std::{ffi::CString, mem::ManuallyDrop, sync::Arc, time::Duration};

use ash::vk;
use gpu_allocator::{MemoryLocation, vulkan::AllocationCreateDesc};

use crate::gpu::{
    Gpu,
    alloc::{
        buffer::Buffer,
        guard::{Resource, ResourceKind},
        image::Image,
    },
};

pub struct BufferAllocDesc<'a> {
    pub name: &'a str,
    pub size: u64,
    pub usage: vk::BufferUsageFlags,
    pub flags: vk::BufferCreateFlags,
    pub location: MemoryLocation,
}

pub struct ImageAllocDesc<'a> {
    pub name: &'a str,
    pub image_type: vk::ImageType,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub samples: vk::SampleCountFlags,
    pub tiling: vk::ImageTiling,
    pub usage: vk::ImageUsageFlags,
    pub sharing_mode: vk::SharingMode,
    pub initial_layout: vk::ImageLayout,
    pub location: MemoryLocation,
}

impl Gpu {
    pub fn alloc_buffer(self: &Arc<Self>, desc: &BufferAllocDesc<'_>) -> anyhow::Result<Buffer> {
        unsafe {
            let buffer = self.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(desc.size)
                    .flags(desc.flags)
                    .usage(desc.usage | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS),
                None,
            )?;
            let requirements = self.get_buffer_memory_requirements(buffer);

            let allocation = self.allocator.lock().allocate(&AllocationCreateDesc {
                name: desc.name,
                requirements,
                location: desc.location,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })?;
            self.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;

            let addr = self
                .get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(buffer));

            Ok(Buffer {
                buffer,
                addr,
                size: desc.size,
                resource: ManuallyDrop::new(Resource {
                    gpu: Arc::clone(self),
                    kind: ResourceKind::Buffer(buffer),
                    allocation: ManuallyDrop::new(allocation),
                }),
            })
        }
    }

    pub fn alloc_upload_buffer(self: &Arc<Self>, size: u64) -> anyhow::Result<Buffer> {
        self.alloc_buffer(&BufferAllocDesc {
            name: "generic_upload_buffer",
            size,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            flags: vk::BufferCreateFlags::empty(),
            location: MemoryLocation::CpuToGpu,
        })
    }

    pub fn alloc_image(self: &Arc<Self>, desc: &ImageAllocDesc<'_>) -> anyhow::Result<Image> {
        unsafe {
            let image = self.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(desc.image_type)
                    .format(desc.format)
                    .extent(desc.extent)
                    .mip_levels(desc.mip_levels)
                    .array_layers(desc.array_layers)
                    .samples(desc.samples)
                    .tiling(desc.tiling)
                    .usage(desc.usage)
                    .sharing_mode(desc.sharing_mode)
                    .initial_layout(desc.initial_layout),
                None,
            )?;

            #[cfg(debug_assertions)]
            {
                let name_cstr = CString::new(desc.name)?;
                self.debug_loader.set_debug_utils_object_name(
                    &vk::DebugUtilsObjectNameInfoEXT::default()
                        .object_handle(image)
                        .object_name(&name_cstr),
                )?;
            }

            let requirements = self.get_image_memory_requirements(image);

            let allocation = self.allocator.lock().allocate(&AllocationCreateDesc {
                name: desc.name,
                requirements,
                location: desc.location,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })?;
            self.bind_image_memory(image, allocation.memory(), allocation.offset())?;
            Ok(Image {
                image,
                resource: ManuallyDrop::new(Resource {
                    gpu: Arc::clone(self),
                    kind: ResourceKind::Image(image),
                    allocation: ManuallyDrop::new(allocation),
                }),
                mip_levels: desc.mip_levels,
                array_layers: desc.array_layers,
                current_state: (
                    desc.initial_layout,
                    vk::PipelineStageFlags::ALL_GRAPHICS,
                    vk::AccessFlags::empty(),
                ),
            })
        }
    }

    pub fn bin_resource<T: 'static + Send>(&self, resource: T) {
        self.bin
            .try_lock_for(Duration::from_secs(5))
            .expect("Failed to acquire bin lock")
            .push((Box::new(resource), 4));
    }
}

impl Gpu {
    pub fn memory_stats(&self) -> MemoryStats {
        let mut heap_budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut properties =
            vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut heap_budget);
        unsafe {
            self.instance
                .get_physical_device_memory_properties2(self.physical_device, &mut properties);
        }

        let total_bytes_used = heap_budget.heap_usage.iter().sum();

        let mut r = MemoryStats {
            total_bytes_used,
            // descriptor_heap_used: resource_heap.used(),
            // descriptor_heap_capacity: resource_heap.capacity(),
            num_descriptor_ring_allocations: self.previous_frame().descriptors.num_allocations(),
            descriptor_ring_used: self.previous_frame().descriptors.used(),
            descriptor_ring_capacity: self.previous_frame().descriptors.capacity(),
            num_upload_ring_allocations: self.previous_frame().upload.num_allocations(),
            upload_ring_used: self.previous_frame().upload.used(),
            upload_ring_capacity: self.previous_frame().upload.capacity(),
            errors: MemoryReportCategories::empty(),
            warnings: MemoryReportCategories::empty(),
            high_water: false,
        };

        // let descriptor_heap_used_ratio =
        //     r.descriptor_heap_used as f64 / r.descriptor_heap_capacity as f64;
        let descriptor_ring_used_ratio =
            r.descriptor_ring_used as f64 / r.descriptor_ring_capacity as f64;
        let upload_ring_used_ratio = r.upload_ring_used as f64 / r.upload_ring_capacity as f64;

        // if descriptor_heap_used_ratio >= 1.0 {
        //     r.errors |= MemoryReportCategories::DESCRIPTOR_HEAP;
        // } else if descriptor_heap_used_ratio >= 0.8 {
        //     r.warnings |= MemoryReportCategories::DESCRIPTOR_HEAP;
        // }

        // if descriptor_heap_used_ratio >= 0.9 {
        //     r.high_water = true;
        // }

        if descriptor_ring_used_ratio >= 1.0 {
            r.errors |= MemoryReportCategories::DESCRIPTOR_RING;
        } else if descriptor_ring_used_ratio >= 0.8 {
            r.warnings |= MemoryReportCategories::DESCRIPTOR_RING;
        }

        if descriptor_ring_used_ratio >= 0.9 {
            r.high_water = true;
        }

        if upload_ring_used_ratio >= 1.0 {
            r.errors |= MemoryReportCategories::UPLOAD_RING;
        } else if upload_ring_used_ratio >= 0.8 {
            r.warnings |= MemoryReportCategories::UPLOAD_RING;
        }

        if upload_ring_used_ratio >= 0.9 {
            r.high_water = true;
        }

        r
    }
}

pub struct MemoryStats {
    pub total_bytes_used: u64,

    // pub descriptor_heap_used: usize,
    // pub descriptor_heap_capacity: usize,
    pub num_descriptor_ring_allocations: usize,
    pub descriptor_ring_used: usize,
    pub descriptor_ring_capacity: usize,

    pub num_upload_ring_allocations: usize,
    pub upload_ring_used: usize,
    pub upload_ring_capacity: usize,

    pub warnings: MemoryReportCategories,
    pub errors: MemoryReportCategories,
    pub high_water: bool,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct MemoryReportCategories: u8 {
        const ALLOCATOR = 1 << 0;
        const DESCRIPTOR_HEAP = 1 << 1;

        const DESCRIPTOR_RING = 1 << 2;
        const UPLOAD_RING = 1 << 3;
    }
}
