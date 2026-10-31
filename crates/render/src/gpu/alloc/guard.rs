use std::{ffi::CString, mem::ManuallyDrop, sync::Arc};

use ash::vk;

use crate::gpu::Gpu;

pub enum ResourceKind {
    Image(vk::Image),
    Buffer(vk::Buffer),
}

pub struct Resource {
    pub(super) gpu: Arc<Gpu>,
    pub(super) kind: ResourceKind,
    pub(super) allocation: ManuallyDrop<gpu_allocator::vulkan::Allocation>,
}

impl Resource {
    pub fn gpu(&self) -> &Arc<Gpu> {
        &self.gpu
    }

    pub fn set_debug_name<A: AsRef<str>>(&self, name: A) {
        let name_cstr = CString::new(name.as_ref()).unwrap();
        let mut info = vk::DebugUtilsObjectNameInfoEXT::default().object_name(&name_cstr);

        match &self.kind {
            ResourceKind::Image(image) => info = info.object_handle(*image),
            ResourceKind::Buffer(buffer) => info = info.object_handle(*buffer),
        }

        #[cfg(debug_assertions)]
        unsafe {
            self.gpu.debug_loader.set_debug_utils_object_name(&info);
        };
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        unsafe {
            self.gpu
                .allocator
                .lock()
                .free(ManuallyDrop::take(&mut self.allocation))
                .unwrap();
            match &self.kind {
                ResourceKind::Image(image) => self.gpu.device.destroy_image(*image, None),
                ResourceKind::Buffer(buffer) => self.gpu.device.destroy_buffer(*buffer, None),
            }
        }
    }
}
