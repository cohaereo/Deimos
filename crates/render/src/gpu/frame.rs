use std::{sync::atomic::AtomicU64, time::Duration};

use anyhow::Context;
use ash::vk;

use crate::gpu::{
    alloc::{descriptor::DescriptorHeap, upload::UploadHeap},
    fence::GpuFence,
};

/// Represents a frame in flight.
pub struct FrameContext {
    // pub stream: FrameCommandStream,
    fence_value: AtomicU64,

    pub upload: UploadHeap,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptors: DescriptorHeap,
    // pub profiler: FrameProfiler,
}

impl FrameContext {
    pub fn new(
        device: ash::Device,
        upload_memory_type_index: u32,
        descriptor_offset_alignment: u64,
    ) -> anyhow::Result<Self> {
        let mb = 1024 * 1024;
        Ok(Self {
            upload: UploadHeap::new(device.clone(), upload_memory_type_index, 32 * mb)
                .context("Failed to allocate UploadHeap")?,
            descriptor_pool: unsafe {
                device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(512)
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
            // stream: FrameCommandStream::new(device.clone()),
            // command_list: NativeCommandList::new(device, d3d12::CommandListType::Direct)?,
            fence_value: AtomicU64::new(0),
            // descriptors: DescriptorRing::new(
            //     device,
            //     d3d12::DescriptorHeapType::CbvSrvUav,
            //     512_000,
            //     true,
            // )?,
            // profiler: FrameProfiler::new(device, allocator)?,
        })
    }

    /// Resets upload rings.
    ///
    /// Keep in mind that this does not reset the command list.
    pub fn begin_frame(&self, device: &ash::Device) {
        self.upload.reset();
        self.descriptors.reset();

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
