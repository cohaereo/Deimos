use std::{ops::Range, sync::Arc};

use ash::vk;
use deimos_data::tfx::ShaderStage;
use gpu_allocator::MemoryLocation;

use crate::gpu::{
    Gpu,
    alloc::{BufferAllocDesc, buffer::Buffer},
    command_list::{BufferResource, CommandList, ConstantBufferView},
};

// pub struct DynamicBuffer {
//     gpu: Arc<Gpu>,
//     // gpu_allocator's Resource owns both the ID3D12Resource and the Allocation
//     resource: Option<gpu_allocator::d3d12::Resource>,
//     mapped_ptr: Option<*mut u8>,
//     size: u64,
// }

// // SAFETY: The mapped pointer is only written from one thread at a time (per-frame).
// // The ID3D12Resource and Allocation are refcounted COM objects / heap-backed.
// unsafe impl Send for DynamicBuffer {}
// unsafe impl Sync for DynamicBuffer {}

// impl DynamicBuffer {
//     /// Create a GPU-only buffer (not CPU-accessible).
//     /// Data must be uploaded via a copy command from an upload buffer.
//     pub fn new_gpu(gpu: &Arc<Gpu>, size: u64, name: &str) -> anyhow::Result<Self> {
//         Self::new_impl(gpu, size, MemoryLocation::GpuOnly, name)
//     }

//     /// Create a CPU-writable upload buffer.
//     /// Stays mapped for the duration of it's lifetime
//     pub fn new_upload(gpu: &Arc<Gpu>, size: u64, name: &str) -> anyhow::Result<Self> {
//         let mut buf = Self::new_impl(gpu, size, MemoryLocation::CpuToGpu, name)?;

//         // Map the upload buffer once and keep it mapped (standard D3D12 pattern)
//         let mut mapped_ptr = std::ptr::null_mut();
//         unsafe {
//             buf.d3d12_resource_win()
//                 .Map(0, None, Some(&mut mapped_ptr))
//                 .context("Failed to map upload buffer")?;
//         }
//         buf.mapped_ptr = Some(mapped_ptr.cast::<u8>());

//         Ok(buf)
//     }

//     fn new_impl(
//         gpu: &Arc<Gpu>,
//         size: u64,
//         location: MemoryLocation,
//         name: &str,
//     ) -> anyhow::Result<Self> {
//         let resource_desc = D3D12_RESOURCE_DESC {
//             Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
//             Width: size,
//             Height: 1,
//             DepthOrArraySize: 1,
//             MipLevels: 1,
//             Format: d3d12::Format::Unknown.into(),
//             SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
//                 Count: 1,
//                 Quality: 0,
//             },
//             Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
//             ..Default::default()
//         };

//         let initial_state = match location {
//             MemoryLocation::CpuToGpu => D3D12_RESOURCE_STATE_GENERIC_READ,
//             _ => D3D12_RESOURCE_STATE_COMMON,
//         };

//         let resource = gpu
//             .allocator
//             .lock()
//             .create_resource(&ResourceCreateDesc {
//                 name,
//                 memory_location: location,
//                 resource_category: ResourceCategory::Buffer,
//                 resource_desc: &resource_desc,
//                 castable_formats: &[],
//                 clear_value: None,
//                 initial_state_or_layout: ResourceStateOrBarrierLayout::ResourceState(initial_state),
//                 resource_type: &ResourceType::Placed,
//             })
//             .catch_device_removal(gpu)?;

//         Ok(Self {
//             gpu: Arc::clone(gpu),
//             resource: Some(resource),
//             mapped_ptr: None,
//             size,
//         })
//     }

//     fn d3d12_resource_win(&self) -> &ID3D12Resource {
//         self.resource.as_ref().unwrap().resource()
//     }

//     pub fn d3d12_resource(&self) -> &Resource {
//         self.d3d12_resource_win().as_ref()
//     }

//     pub fn gpu_virtual_address(&self) -> GpuVirtualAddress {
//         self.d3d12_resource().gpu_virtual_address()
//     }

//     pub const fn size(&self) -> u64 {
//         self.size
//     }

//     /// Write data into an upload buffer. Panics if this is not an upload buffer.
//     pub fn write(&self, offset: u64, data: &[u8]) {
//         let ptr = self
//             .mapped_ptr
//             .expect("Buffer::write called on a non-upload buffer");
//         assert!(
//             offset + data.len() as u64 <= self.size,
//             "Write out of bounds"
//         );
//         unsafe {
//             std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset as usize), data.len());
//         }
//     }

//     pub fn vb_view(&self, stride: u32) -> d3d12::VertexBufferView {
//         d3d12::VertexBufferView::new(self.gpu_virtual_address(), self.size as u32, stride)
//     }
// }

// impl Drop for DynamicBuffer {
//     fn drop(&mut self) {
//         // Unmap if mapped
//         if self.mapped_ptr.is_some() {
//             unsafe {
//                 self.d3d12_resource_win().Unmap(0, None);
//             }
//             self.mapped_ptr = None;
//         }
//         // Return the resource + allocation to the allocator
//         if let Some(resource) = self.resource.take()
//             && let Err(e) = self.gpu.allocator.lock().free_resource(resource)
//         {
//             error!("Failed to free buffer: {:?}", e);
//         }
//     }
// }

/// Immutable GPU-only buffer
pub struct ImmutableBuffer {
    gpu: Arc<Gpu>,
    buffer: Buffer,
    view: vk::BufferView,
    size: u64,
    view_format: vk::Format,
}

impl ImmutableBuffer {
    pub fn new(
        gpu: &Arc<Gpu>,
        name: &str,
        view_format: vk::Format,
        data: &[u8],
    ) -> anyhow::Result<Self> {
        let buffer = gpu.alloc_buffer(&BufferAllocDesc {
            name,
            size: data.len() as u64,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::TRANSFER_DST
                | vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER,
            flags: vk::BufferCreateFlags::empty(),
            location: MemoryLocation::GpuOnly,
        })?;

        let upload_buffer = gpu.alloc_upload_buffer(data.len() as u64)?;
        unsafe {
            upload_buffer
                .mapped_ptr()
                .as_ptr()
                .copy_from_nonoverlapping(data.as_ptr(), data.len());
        }

        let cmd = gpu.upload_command_buffer();
        unsafe {
            gpu.cmd_copy_buffer(
                *cmd,
                upload_buffer.vk_buffer(),
                buffer.vk_buffer(),
                &[vk::BufferCopy::default().size(data.len() as u64)],
            );
        }

        let view = unsafe {
            gpu.device.create_buffer_view(
                &vk::BufferViewCreateInfo::default()
                    .buffer(buffer.vk_buffer())
                    .format(view_format)
                    .range(vk::WHOLE_SIZE),
                None,
            )?
        };

        Ok(Self {
            gpu: Arc::clone(gpu),
            buffer,
            view,
            view_format,
            size: data.len() as u64,
        })
    }

    pub fn create_srv(&self, bytes_range: Range<u64>) -> vk::BufferView {
        unsafe {
            self.gpu
                .device
                .create_buffer_view(
                    &vk::BufferViewCreateInfo::default()
                        .buffer(self.buffer.vk_buffer())
                        .format(self.view_format)
                        .offset(bytes_range.start)
                        .range(bytes_range.end - bytes_range.start),
                    None,
                )
                .expect("Failed to create buffer view")
        }
    }

    pub const fn buffer_view(&self) -> vk::BufferView {
        self.view
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn cbv(&self) -> ConstantBufferView {
        self.buffer.cbv()
    }

    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub const fn srv(&self, offset: u64, size: u64) -> BufferResource {
        BufferResource {
            addr: self.buffer.device_address() + offset,
            size,
            format: self.view_format,
        }
    }

    pub fn bind_srv(&self, cmd: &mut CommandList, stage: ShaderStage, slot: u32) {
        cmd.set_shader_resource_buffer(stage, slot, Some(self.srv(0, self.size)));
    }

    pub fn bind_cbv(&self, cmd: &mut CommandList, stage: ShaderStage, slot: u32) {
        cmd.set_shader_constant_buffer_view(stage, slot, Some(self.cbv()));
    }
}

impl Drop for ImmutableBuffer {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_buffer_view(self.view, None);
        }
    }
}
