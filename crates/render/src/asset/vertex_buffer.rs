use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use deimos_data::tfx::buffers::VertexBufferHeader;
use gpu_allocator::MemoryLocation;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::gpu::{
    Gpu,
    alloc::{BufferAllocDesc, buffer::Buffer},
};

pub struct VertexBuffer {
    buffer: Buffer,

    pub size: u32,
    pub length: u32,
    pub stride: u32,
}

impl VertexBuffer {
    pub fn get_raw_data_and_stride(hash: TagHash) -> anyhow::Result<(Vec<u8>, u32)> {
        let entry = package_manager()
            .get_entry(hash)
            .context("Entry not found")?;

        let header: VertexBufferHeader = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read header data")?;
        let data = package_manager()
            .read_tag(entry.reference)
            .context("Failed to read buffer data")?;

        Ok((data, header.stride as u32))
    }

    pub fn load(gpu: &Arc<Gpu>, hash: TagHash) -> anyhow::Result<Self> {
        let (data, stride) = Self::get_raw_data_and_stride(hash)?;

        let vb = Self::load_data(gpu, &data, stride)?;
        vb.buffer
            .resource()
            .set_debug_name(format!("vertex_buffer {hash}"));
        Ok(vb)
    }

    pub fn load_data(gpu: &Arc<Gpu>, data: &[u8], stride: u32) -> anyhow::Result<Self> {
        Self::load_data_ex(gpu, data, stride)
    }

    pub fn load_data_ex(gpu: &Arc<Gpu>, data: &[u8], stride: u32) -> anyhow::Result<Self> {
        let buffer = gpu.alloc_buffer(&BufferAllocDesc {
            name: "generic_vertex_buffer",
            size: data.len() as u64,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
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

        Ok(Self {
            buffer,
            size: data.len() as u32,
            length: data.len() as u32 / stride,
            stride,
        })
    }

    pub fn bind_single(&self, cmd: &vk::CommandBuffer, slot: u32) {
        unsafe {
            self.buffer().resource().gpu().cmd_bind_vertex_buffers(
                *cmd,
                slot,
                &[self.buffer.vk_buffer()],
                &[0],
            );
        }
    }

    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}
