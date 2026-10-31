use std::{ffi::c_void, mem::ManuallyDrop, ptr::NonNull};

use ash::vk;

use crate::gpu::{
    alloc::guard::Resource,
    command_list::{BufferResource, ConstantBufferView},
};

pub struct Buffer {
    pub(super) buffer: vk::Buffer,
    pub(super) addr: vk::DeviceAddress,
    pub(super) size: u64,
    pub(super) resource: ManuallyDrop<Resource>,
}

impl Buffer {
    pub fn mapped_ptr(&self) -> NonNull<u8> {
        self.resource
            .allocation
            .mapped_ptr()
            .expect("Buffer is not host-visible")
            .cast()
    }

    pub fn mapped_slice(&self) -> &[u8] {
        self.resource
            .allocation
            .mapped_slice()
            .expect("Buffer is not host-visible")
    }

    pub fn cbv(&self) -> ConstantBufferView {
        ConstantBufferView {
            addr: self.addr,
            size: self.size,
        }
    }

    pub fn vk_buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub const fn device_address(&self) -> vk::DeviceAddress {
        self.addr
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn resource(&self) -> &Resource {
        &self.resource
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let gpu = self.resource.gpu.clone();
        unsafe {
            gpu.bin_resource(ManuallyDrop::take(&mut self.resource));
        }
    }
}
