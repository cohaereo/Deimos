use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Context;
use ash::vk;
use parking_lot::Mutex;

use crate::gpu::{
    alloc::{descriptor::DescriptorHeap, upload::UploadHeap},
    fence::GpuFence,
    profiler::FrameProfiler,
};

const MAX_SECONDARY_COMMAND_BUFFERS_PER_POOL: usize = 256;

struct CommandPoolSlot {
    pool: vk::CommandPool,
    buffers: Mutex<Vec<vk::CommandBuffer>>,
    next: AtomicUsize,
}

pub struct SecondaryCommandBuffers {
    device: ash::Device,
    slots: Vec<CommandPoolSlot>,
}

impl SecondaryCommandBuffers {
    fn new(
        device: &ash::Device,
        queue_family_index: u32,
        pool_count: usize,
    ) -> anyhow::Result<Self> {
        let slots = (0..pool_count.max(1))
            .map(|_| {
                let pool = unsafe {
                    device.create_command_pool(
                        &vk::CommandPoolCreateInfo::default()
                            .queue_family_index(queue_family_index)
                            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                        None,
                    )
                }?;

                Ok(CommandPoolSlot {
                    pool,
                    buffers: Mutex::new(Vec::new()),
                    next: AtomicUsize::new(0),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self {
            device: device.clone(),
            slots,
        })
    }

    pub const fn pool_count(&self) -> usize {
        self.slots.len()
    }

    /// Resets all pools. The GPU must not be using the buffers.
    pub fn reset(&self) {
        for slot in &self.slots {
            unsafe {
                self.device
                    .reset_command_pool(slot.pool, vk::CommandPoolResetFlags::empty())
                    .ok();
            }
            slot.next.store(0, Ordering::Relaxed);
        }
    }

    /// Returns the next free secondary command buffer for `pool_index`
    ///
    /// Returns `None` when the owning thread uses more buffers than the limit for one frame
    pub fn acquire(&self, pool_index: usize) -> anyhow::Result<Option<vk::CommandBuffer>> {
        let slot = &self.slots[pool_index];
        let index = slot.next.fetch_add(1, Ordering::Relaxed);
        if index >= MAX_SECONDARY_COMMAND_BUFFERS_PER_POOL {
            return Ok(None);
        }

        let mut buffers = slot.buffers.lock();
        if index >= buffers.len() {
            let count = (index + 1 - buffers.len()) as u32;
            let allocated = unsafe {
                self.device.allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(slot.pool)
                        .level(vk::CommandBufferLevel::SECONDARY)
                        .command_buffer_count(count),
                )
            }?;
            buffers.extend(allocated);
        }

        Ok(Some(buffers[index]))
    }

    pub fn reset_buffer(&self, buffer: vk::CommandBuffer) -> anyhow::Result<()> {
        unsafe {
            self.device
                .reset_command_buffer(buffer, vk::CommandBufferResetFlags::empty())?;
        }
        Ok(())
    }

    pub fn begin_buffer(
        &self,
        buffer: vk::CommandBuffer,
        begin_info: &vk::CommandBufferBeginInfo<'_>,
    ) -> anyhow::Result<()> {
        unsafe { self.device.begin_command_buffer(buffer, begin_info)? };
        Ok(())
    }

    pub fn end_buffer(&self, buffer: vk::CommandBuffer) -> anyhow::Result<()> {
        unsafe { self.device.end_command_buffer(buffer)? };
        Ok(())
    }
}

impl Drop for SecondaryCommandBuffers {
    fn drop(&mut self) {
        unsafe {
            for slot in &self.slots {
                self.device.destroy_command_pool(slot.pool, None);
            }
        }
    }
}

/// Represents a frame in flight.
pub struct FrameContext {
    fence_value: AtomicU64,

    pub upload: UploadHeap,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptors: DescriptorHeap,
    pub secondary: SecondaryCommandBuffers,
    pub profiler: FrameProfiler,
}

impl FrameContext {
    pub fn new(
        device: ash::Device,
        upload_memory_type_index: u32,
        descriptor_offset_alignment: u64,
        queue_family_index: u32,
        secondary_pool_count: usize,
        timestamp_period: f32,
    ) -> anyhow::Result<Self> {
        let mb = 1024 * 1024;
        let secondary =
            SecondaryCommandBuffers::new(&device, queue_family_index, secondary_pool_count)
                .context("Failed to allocate secondary command buffers")?;
        let profiler = FrameProfiler::new(&device, timestamp_period)
            .context("Failed to create frame profiler")?;

        Ok(Self {
            upload: UploadHeap::new(device.clone(), upload_memory_type_index, 48 * mb)
                .context("Failed to allocate UploadHeap")?,
            descriptor_pool: unsafe {
                device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(16384)
                        .pool_sizes(&[vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            descriptor_count: 512,
                        }]),
                    None,
                )?
            },
            descriptors: DescriptorHeap::new(
                device,
                upload_memory_type_index,
                32 * mb,
                descriptor_offset_alignment,
            )
            .context("Failed to allocate DescriptorRing")?,
            secondary,
            profiler,
            fence_value: AtomicU64::new(0),
        })
    }

    /// Resets upload rings.
    ///
    /// Keep in mind that this does not reset the command list.
    pub fn begin_frame(&self, device: &ash::Device) {
        self.upload.reset();
        self.descriptors.reset();
        self.secondary.reset();
        self.profiler.begin_frame();

        unsafe {
            device
                .reset_descriptor_pool(
                    self.descriptor_pool,
                    vk::DescriptorPoolResetFlags::default(),
                )
                .ok();
        }

        // self.profiler.begin_frame(queue);
    }

    pub fn wait_for_completion(&self, fence: &GpuFence) -> anyhow::Result<()> {
        let fence_value = self.fence_value.load(std::sync::atomic::Ordering::Acquire);
        fence.wait(fence_value, Some(Duration::from_secs(5)))?;
        Ok(())
    }

    pub fn signal(&self, fence: &GpuFence, queue: &vk::Queue) -> anyhow::Result<()> {
        let fence_value = fence.signal(queue)?;
        self.fence_value
            .store(fence_value, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}
