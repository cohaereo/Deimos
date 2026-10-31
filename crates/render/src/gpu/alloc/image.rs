use std::mem::ManuallyDrop;

use ash::vk;

use crate::gpu::alloc::guard::Resource;

pub struct Image {
    pub(super) image: vk::Image,
    pub(super) resource: ManuallyDrop<Resource>,

    pub(super) mip_levels: u32,
    pub(super) array_layers: u32,
    pub(super) current_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
}

impl Image {
    pub fn vk_image(&self) -> vk::Image {
        self.image
    }

    pub fn current_state(&self) -> (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) {
        self.current_state
    }

    pub fn current_layout(&self) -> vk::ImageLayout {
        self.current_state.0
    }

    pub fn layout_barrier(
        &mut self,
        cmd: vk::CommandBuffer,
        aspect_mask: vk::ImageAspectFlags,
        new_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        if new_state == self.current_state {
            return;
        }

        let (new_layout, new_stage, new_access) = new_state;
        let (current_layout, current_stage, current_access) = self.current_state;

        self.resource.gpu.cmd_image_layout_barrier_range(
            cmd,
            self.image,
            vk::ImageSubresourceRange::default()
                .aspect_mask(aspect_mask)
                .base_mip_level(0)
                .level_count(self.mip_levels)
                .base_array_layer(0)
                .layer_count(self.array_layers),
            current_layout,
            new_layout,
            current_stage,
            new_stage,
            current_access,
            new_access,
        );
        self.current_state = new_state;
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        let gpu = self.resource.gpu.clone();
        unsafe {
            gpu.bin_resource(ManuallyDrop::take(&mut self.resource));
        }
    }
}
