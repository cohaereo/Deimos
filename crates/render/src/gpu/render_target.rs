use std::sync::Arc;

use ash::vk;
use gpu_allocator::MemoryLocation;

use crate::gpu::{
    Gpu,
    alloc::{ImageAllocDesc, image::Image},
    command_buffer::IMAGE_STATE_TRANSFER_DST,
};

pub struct RenderTarget {
    gpu: Arc<Gpu>,

    pub image: Image,
    view: vk::ImageView,
    size: (u32, u32),

    name: String,
    format: vk::Format,
    view_format: vk::Format,
}

impl RenderTarget {
    pub fn new(
        gpu: &Arc<Gpu>,
        name: &str,
        format: vk::Format,
        view_format: vk::Format,
        (width, height): (u32, u32),
    ) -> anyhow::Result<Self> {
        let image = gpu.alloc_image(&ImageAllocDesc {
            name,
            image_type: vk::ImageType::TYPE_2D,
            format,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            location: MemoryLocation::GpuOnly,
        })?;

        let view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(view_format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        }?;

        Ok(Self {
            gpu: gpu.clone(),
            image,
            view,
            size: (width, height),

            name: name.to_string(),
            format,
            view_format,
        })
    }

    pub const fn output_format(&self) -> vk::Format {
        self.view_format
    }

    pub const fn view(&self) -> vk::ImageView {
        self.view
    }

    pub const fn resolution(&self) -> (u32, u32) {
        self.size
    }

    pub fn transition(
        &mut self,
        cmd: &vk::CommandBuffer,
        new_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        self.image
            .layout_barrier(*cmd, vk::ImageAspectFlags::COLOR, new_state);
    }

    pub fn clear(&mut self, cmd: &vk::CommandBuffer, color: [f32; 4]) {
        let old_state = self.image.current_state();
        if old_state.0 != vk::ImageLayout::TRANSFER_DST_OPTIMAL {
            self.transition(cmd, IMAGE_STATE_TRANSFER_DST);
        }

        unsafe {
            self.gpu.cmd_clear_color_image(
                *cmd,
                self.image.vk_image(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &vk::ClearColorValue { float32: color },
                &[vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .level_count(1)],
            );
        }

        self.transition(cmd, old_state);
    }

    /// Resize the depth buffer.
    ///
    /// # Remarks
    /// - Won't resize if the new size is the same as the current size.
    /// - The old depth buffer is put into the GPU resource bin, meaning it won't actually be destroyed until it's no longer used
    pub fn resize(&mut self, (width, height): (u32, u32)) -> anyhow::Result<()> {
        if width == self.size.0 && height == self.size.1 {
            return Ok(());
        }

        let mut new_depth_buffer = Self::new(
            &self.gpu,
            &self.name,
            self.format,
            self.view_format,
            (width, height),
        )?;
        std::mem::swap(self, &mut new_depth_buffer);

        Ok(())
    }

    pub fn take(&mut self) -> anyhow::Result<Self> {
        let new = Self::new(
            &self.gpu,
            &self.name,
            self.format,
            self.view_format,
            self.size,
        )?;

        Ok(std::mem::replace(self, new))
    }
}

impl Drop for RenderTarget {
    fn drop(&mut self) {
        unsafe {
            self.gpu.destroy_image_view(self.view, None);
        }
    }
}

pub struct DepthBuffer {
    gpu: Arc<Gpu>,
    pub image: Image,
    depth_stencil_view: vk::ImageView,
    depth_view: vk::ImageView,
    size: (u32, u32),

    output_format: vk::Format,
}

impl DepthBuffer {
    pub fn new(gpu: &Arc<Gpu>, (width, height): (u32, u32)) -> anyhow::Result<Self> {
        let image = gpu.alloc_image(&ImageAllocDesc {
            name: "depth_buffer",
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::D32_SFLOAT_S8_UINT,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            location: MemoryLocation::GpuOnly,
        })?;

        let depth_stencil_view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::D32_SFLOAT_S8_UINT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(
                                vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                            )
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        }?;

        let depth_view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::D32_SFLOAT_S8_UINT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        }?;

        Ok(Self {
            gpu: gpu.clone(),
            image,
            depth_stencil_view,
            depth_view,
            size: (width, height),
            output_format: vk::Format::D32_SFLOAT_S8_UINT,
        })
    }

    pub const fn output_format(&self) -> vk::Format {
        self.output_format
    }

    pub fn transition(
        &mut self,
        cmd: &vk::CommandBuffer,
        new_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        self.image.layout_barrier(
            *cmd,
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
            new_state,
        );
    }

    /// Returns a view to use as a depth/stencil attachment
    pub const fn depth_stencil_view(&self) -> vk::ImageView {
        self.depth_stencil_view
    }

    /// Returns a depth-only view for sampling in shaders
    pub const fn depth_view(&self) -> vk::ImageView {
        self.depth_view
    }

    /// Resize the depth buffer.
    ///
    /// # Remarks
    /// - Won't resize if the new size is the same as the current size.
    /// - The old depth buffer is put into the GPU resource bin, meaning it won't actually be destroyed until it's no longer used
    pub fn resize(&mut self, (width, height): (u32, u32)) -> anyhow::Result<()> {
        if width == self.size.0 && height == self.size.1 {
            return Ok(());
        }

        let mut new_depth_buffer = Self::new(&self.gpu, (width, height))?;
        std::mem::swap(self, &mut new_depth_buffer);

        Ok(())
    }

    pub fn clear(&mut self, cmd: &vk::CommandBuffer, depth: f32, stencil: u8) {
        let old_state = self.image.current_state();
        if old_state.0 != vk::ImageLayout::TRANSFER_DST_OPTIMAL {
            self.transition(cmd, IMAGE_STATE_TRANSFER_DST);
        }

        unsafe {
            self.gpu.cmd_clear_depth_stencil_image(
                *cmd,
                self.image.vk_image(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &vk::ClearDepthStencilValue {
                    depth,
                    stencil: stencil as u32,
                },
                &[vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    .layer_count(1)
                    .level_count(1)],
            );
        }

        self.transition(cmd, old_state);
    }

    pub const fn resolution(&self) -> (u32, u32) {
        self.size
    }
}
