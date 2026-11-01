use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::Result;
use ash::vk;

/// A fence wrapper that handles signaling and waiting
pub struct GpuFence {
    device: ash::Device,
    semaphore: vk::Semaphore,

    next_value: AtomicU64,
}

impl GpuFence {
    /// Creates a new `AutoFence`
    pub fn new(device: &ash::Device) -> Result<Self> {
        let mut semaphore_type = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let semaphore = unsafe {
            device.create_semaphore(
                &vk::SemaphoreCreateInfo::default().push_next(&mut semaphore_type),
                None,
            )?
        };

        Ok(Self {
            device: device.clone(),
            semaphore,

            next_value: AtomicU64::new(1),
        })
    }

    /// Waits for the GPU to reach the previously set fence value
    pub fn wait(&self, value: u64, timeout: Option<Duration>) -> Result<()> {
        let current = unsafe { self.device.get_semaphore_counter_value(self.semaphore)? };
        if current >= value {
            return Ok(());
        }

        let timeout_ns = timeout.map_or(u64::MAX, |t| t.as_nanos() as u64);

        unsafe {
            self.device.wait_semaphores(
                &vk::SemaphoreWaitInfo::default()
                    .semaphores(&[self.semaphore])
                    .values(&[value]),
                timeout_ns,
            )?;
        }

        Ok(())
    }

    /// Queues a signal for the fence and returns the key. Calling await will block until the given signal key has been reached.
    pub fn signal(&self, queue: &vk::Queue) -> Result<u64> {
        let value = self.next_value.fetch_add(1, Ordering::Relaxed);
        let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .signal_semaphore_values(std::slice::from_ref(&value));
        unsafe {
            self.device.queue_submit(
                *queue,
                &[vk::SubmitInfo::default()
                    .signal_semaphores(&[self.semaphore])
                    .push_next(&mut timeline_info)],
                vk::Fence::null(),
            )?;
        }
        Ok(value)
    }
}

/// An even simpler GPU fence that tracks the last signal value and waits for it to be reached.
pub struct GpuFenceWaiter {
    fence: GpuFence,
    value: AtomicU64,
}

impl GpuFenceWaiter {
    pub fn new(device: &ash::Device) -> Result<Self> {
        Ok(Self {
            fence: GpuFence::new(device)?,
            value: AtomicU64::new(1),
        })
    }

    pub fn wait_for_previous(&self, timeout: Option<Duration>) -> Result<()> {
        self.fence.wait(self.value.load(Ordering::Relaxed), timeout)
    }

    pub fn wait_for_next(&self, timeout: Option<Duration>) -> Result<()> {
        self.fence
            .wait(self.value.load(Ordering::Relaxed) + 1, timeout)
    }

    pub fn signal(&self, queue: &vk::Queue) -> Result<()> {
        let fence_value = self.fence.signal(queue)?;
        self.value.store(fence_value, Ordering::Relaxed);
        Ok(())
    }
}
