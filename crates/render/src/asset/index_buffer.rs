use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use deimos_data::tfx::buffers::IndexBufferHeader;
use gpu_allocator::MemoryLocation;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::gpu::{
    Gpu,
    alloc::{BufferAllocDesc, buffer::Buffer},
};

pub struct IndexBuffer {
    pub buffer: Buffer,
    /// Amount of elements in the buffer
    pub length: usize,
    pub index_type: vk::IndexType,
    size: usize,
}

impl IndexBuffer {
    pub fn load(gpu: &Arc<Gpu>, hash: TagHash) -> anyhow::Result<Self> {
        let entry = package_manager()
            .get_entry(hash)
            .context("Entry not found")?;

        let header: IndexBufferHeader = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read header data")?;
        let data = package_manager()
            .read_tag(entry.reference)
            .context("Failed to read buffer data")?;

        let vb = Self::load_data(gpu, &data, header.is_32bit)?;

        vb.buffer
            .resource()
            .set_debug_name(format!("index_bufer {hash}"));

        Ok(vb)
    }

    pub fn load_data(gpu: &Arc<Gpu>, data: &[u8], is_32bit: bool) -> anyhow::Result<Self> {
        let buffer = gpu.alloc_buffer(&BufferAllocDesc {
            name: "generic_index_buffer",
            size: data.len() as u64,
            usage: vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
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
            length: data.len() / if is_32bit { 4 } else { 2 },
            size: data.len(),
            index_type: if is_32bit {
                vk::IndexType::UINT32
            } else {
                vk::IndexType::UINT16
            },
        })
    }

    pub fn bind(&self, cmd: &vk::CommandBuffer) {
        unsafe {
            self.buffer.resource().gpu().cmd_bind_index_buffer(
                *cmd,
                self.buffer.vk_buffer(),
                0,
                self.index_type,
            );
        };
        // cmd.input_assembler_set_index_buffer(&self.buffer, self.format, 0);
        // self.cmd.ia_set_index_buffer(
        //     self.resource.resource().gpu_virtual_address(),
        //     self.size as u32,
        //     self.format,
        // );
    }
}
