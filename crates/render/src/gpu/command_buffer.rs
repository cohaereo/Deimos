use ash::vk;
use std::{ops::Deref, sync::Arc};

use crate::gpu::{
    Gpu,
    alloc::image::Image,
    render_target::{DepthBuffer, RenderTarget},
};

pub const IMAGE_STATE_TRANSFER_SRC: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) = (
    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    vk::PipelineStageFlags::TRANSFER,
    vk::AccessFlags::TRANSFER_READ,
);

pub const IMAGE_STATE_TRANSFER_DST: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) = (
    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    vk::PipelineStageFlags::TRANSFER,
    vk::AccessFlags::TRANSFER_WRITE,
);

pub const IMAGE_STATE_SHADER_READ: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) = (
    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    vk::PipelineStageFlags::ALL_GRAPHICS,
    vk::AccessFlags::SHADER_READ,
);

pub const IMAGE_STATE_COLOR_ATTACHMENT: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) = (
    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    vk::PipelineStageFlags::ALL_GRAPHICS,
    vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
);

pub const IMAGE_STATE_DEPTH_ATTACHMENT: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags) = (
    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    vk::PipelineStageFlags::ALL_GRAPHICS,
    vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
);

/// Native Vulkan command buffer wrapper
#[derive(Clone)]
pub struct CommandBuffer {
    gpu: Arc<Gpu>,
    cmd: vk::CommandBuffer,
    // is_open: AtomicBool,
}

impl CommandBuffer {
    pub fn from_native_command_list(gpu: &Arc<Gpu>, cmd: vk::CommandBuffer) -> Self {
        Self {
            gpu: Arc::clone(gpu),
            cmd,
            // is_open: AtomicBool::new(false),
        }
    }
}

// // General operations
// impl CommandBuffer {
//     pub fn new(gpu: &Arc<Gpu>, level: vk::CommandBufferLevel) -> anyhow::Result<Self> {
//         let cmd = unsafe {
//             gpu.device.allocate_command_buffers(
//                 &vk::CommandBufferAllocateInfo::default()
//                     .command_pool(gpu.command_pool)
//                     .level(level)
//                     .command_buffer_count(1),
//             )?
//         }[0];

//         Ok(Self {
//             gpu: Arc::clone(gpu),
//             cmd,
//             is_open: AtomicBool::new(false),
//         })
//     }

//     pub fn is_open(&self) -> bool {
//         self.is_open.load(Ordering::Acquire)
//     }

//     pub fn begin(&self) -> anyhow::Result<()> {
//         assert!(!self.is_open(), "command list is already recording");
//         unsafe {
//             self.gpu
//                 .begin_command_buffer(self.cmd, &vk::CommandBufferBeginInfo::default());
//         }
//         self.is_open.store(true, Ordering::Release);
//         Ok(())
//     }

//     pub fn end(&self) -> anyhow::Result<()> {
//         assert!(self.is_open(), "command list is not recording");
//         unsafe {
//             self.gpu.end_command_buffer(self.cmd)?;
//         }
//         self.is_open.store(false, Ordering::Release);
//         Ok(())
//     }
// }

// Commands (vkCmd*)
impl CommandBuffer {
    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        unsafe {
            self.gpu.cmd_draw(
                self.cmd,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            );
        }
    }

    pub fn draw_indexed(
        &self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.gpu.cmd_draw_indexed(
                self.cmd,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            );
        }
    }

    pub fn set_stencil_reference(&self, face_mask: vk::StencilFaceFlags, reference: u32) {
        unsafe {
            self.gpu
                .cmd_set_stencil_reference(self.cmd, face_mask, reference);
        }
    }

    pub fn set_primitive_topology(&self, topology: vk::PrimitiveTopology) {
        unsafe {
            self.gpu.cmd_set_primitive_topology(self.cmd, topology);
        }
    }

    pub fn set_scissor(&self, first: u32, scissors: &[vk::Rect2D]) {
        unsafe {
            self.gpu.cmd_set_scissor(self.cmd, first, scissors);
        }
    }

    pub fn set_viewport(&self, first: u32, viewports: &[vk::Viewport]) {
        unsafe {
            self.gpu.cmd_set_viewport(self.cmd, first, viewports);
        }
    }

    pub fn set_vertex_buffers(
        &self,
        first: u32,
        buffers: &[vk::Buffer],
        offsets: &[vk::DeviceSize],
    ) {
        unsafe {
            self.gpu
                .cmd_bind_vertex_buffers(self.cmd, first, buffers, offsets);
        }
    }

    pub fn copy_image(&self, src_image: &Image, dst_image: &Image, regions: &[vk::ImageCopy]) {
        unsafe {
            self.gpu.cmd_copy_image(
                self.cmd,
                src_image.vk_image(),
                src_image.current_layout(),
                dst_image.vk_image(),
                dst_image.current_layout(),
                regions,
            );
        }
    }

    pub fn copy_render_target(&self, src: &RenderTarget, dst: &RenderTarget) {
        self.copy_image(
            &src.image,
            &dst.image,
            &[vk::ImageCopy::default()
                .extent(vk::Extent3D {
                    width: dst.resolution().0,
                    height: dst.resolution().1,
                    depth: 1,
                })
                .src_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .dst_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )],
        );
    }

    pub fn copy_depth_buffer(&self, src: &DepthBuffer, dst: &DepthBuffer) {
        self.copy_image(
            &src.image,
            &dst.image,
            &[vk::ImageCopy::default()
                .extent(vk::Extent3D {
                    width: dst.resolution().0,
                    height: dst.resolution().1,
                    depth: 1,
                })
                .src_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .layer_count(1),
                )
                .dst_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .layer_count(1),
                )],
        );
    }

    pub fn begin_rendering(
        &self,
        resolution: (u32, u32),
        color_attachments: &[vk::RenderingAttachmentInfo<'_>],
        depth_attachment: Option<&vk::RenderingAttachmentInfo<'_>>,
        flags: vk::RenderingFlags,
    ) -> anyhow::Result<()> {
        let render_area = vk::Rect2D::default().extent(vk::Extent2D {
            width: resolution.0,
            height: resolution.1,
        });
        let mut rendering_info = vk::RenderingInfo::default()
            .flags(flags)
            .render_area(render_area)
            .layer_count(1)
            .color_attachments(color_attachments);

        if let Some(depth) = depth_attachment {
            rendering_info = rendering_info.depth_attachment(depth);
        }

        unsafe {
            self.gpu.cmd_begin_rendering(self.cmd, &rendering_info);
        }
        Ok(())
    }

    pub fn execute_commands(&self, command_buffers: &[vk::CommandBuffer]) {
        if command_buffers.is_empty() {
            return;
        }

        unsafe {
            self.gpu.cmd_execute_commands(self.cmd, command_buffers);
        }
    }

    pub fn end_rendering(&self) {
        unsafe {
            self.gpu.cmd_end_rendering(self.cmd);
        }
    }
}

// Debug labels (VK_EXT_debug_utils)
impl CommandBuffer {
    pub fn begin_debug_utils_label(&self, label_name: &str, color: [f32; 4]) {
        let label_name_cstr = std::ffi::CString::new(label_name).unwrap();
        #[cfg(debug_assertions)]
        unsafe {
            self.gpu.debug_loader.cmd_begin_debug_utils_label(
                self.cmd,
                &vk::DebugUtilsLabelEXT::default()
                    .label_name(label_name_cstr.as_c_str())
                    .color(color),
            );
        }
    }

    pub fn end_debug_utils_label(&self) {
        #[cfg(debug_assertions)]
        unsafe {
            self.gpu.debug_loader.cmd_end_debug_utils_label(self.cmd);
        }
    }

    /// Creates a new event scope with the given name. The event will automatically end when the returned RAII guard is dropped.
    #[must_use]
    pub fn event_scope(&self, name: impl AsRef<str>, [r, g, b]: [f32; 3]) -> EventSpanGuard {
        self.begin_debug_utils_label(name.as_ref(), [r, g, b, 1.0]);
        EventSpanGuard { this: self.clone() }
    }
}

impl Deref for CommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

// struct CommandListSlot {
//     command_list: CommandBuffer,
//     fence_waiter: Arc<GpuFenceWaiter>,
//     fence_value: u64,
// }

// pub struct AsyncCommandListRing {
//     queue: d3d12::CommandQueue,
//     fence: d3d12::Fence,
//     fence_event: d3d12::WaitableObject,

//     next_fence_value: AtomicU64,
//     slots: Mutex<Box<[CommandListSlot]>>,
//     head: Mutex<usize>,
// }

// impl AsyncCommandListRing {
//     pub fn new(
//         device: &d3d12::Device,
//         queue: d3d12::CommandQueue,
//         capacity: usize,
//     ) -> anyhow::Result<Self> {
//         let fence = device.create_fence(0)?;
//         let fence_event = WaitableObject::new(false, false)?;

//         let slots = (0..capacity)
//             .map(|_| {
//                 let command_list = CommandBuffer::new(device, d3d12::CommandListType::Direct)?;
//                 Ok(CommandListSlot {
//                     command_list,
//                     fence_value: 0,
//                     #[allow(clippy::arc_with_non_send_sync)]
//                     fence_waiter: Arc::new(GpuFenceWaiter::new(device)?),
//                 })
//             })
//             .collect::<anyhow::Result<Box<[_]>>>()?;

//         slots[0].command_list.begin()?;

//         Ok(Self {
//             queue,
//             fence,
//             fence_event,
//             next_fence_value: AtomicU64::new(1),
//             slots: Mutex::new(slots),
//             head: Mutex::new(0),
//         })
//     }

//     pub fn advance(&self) -> anyhow::Result<()> {
//         let mut head = self.head.lock();
//         let mut slots = self.slots.lock();

//         // Finish the current slot's command list and signal the fence
//         let previous_slot = &mut slots[*head];
//         let fence_value = self.next_fence_value.fetch_add(1, Ordering::Relaxed);
//         previous_slot.command_list.end()?;
//         self.queue
//             .execute_command_lists(std::slice::from_ref(&previous_slot.command_list));
//         self.queue.signal(&self.fence, fence_value)?;
//         previous_slot.fence_waiter.signal(&self.queue);
//         previous_slot.fence_value = fence_value;

//         // Advance to next slot and wait for it to be ready (if necessary)
//         *head = (*head + 1) % slots.len();
//         let current_slot = &mut slots[*head];
//         self.wait_for(current_slot.fence_value)?;
//         current_slot.command_list.begin()?;

//         Ok(())
//     }

//     pub fn submit<F>(&self, f: F) -> anyhow::Result<Arc<GpuFenceWaiter>>
//     where
//         F: FnOnce(&CommandBuffer) -> anyhow::Result<()>,
//     {
//         let head = self.head.lock();
//         let mut slots = self.slots.lock();
//         let slot = &mut slots[*head];

//         f(&slot.command_list)?;

//         Ok(slot.fence_waiter.clone())
//     }

//     fn wait_for(&self, fence_value: u64) -> anyhow::Result<()> {
//         if self.fence.get_completed_value() < fence_value {
//             self.fence
//                 .set_event_on_completion(&self.fence_event, fence_value)?;
//             self.fence_event.wait(None);
//         }
//         Ok(())
//     }
// }

pub struct EventSpanGuard {
    this: CommandBuffer,
}

impl Drop for EventSpanGuard {
    fn drop(&mut self) {
        self.this.end_debug_utils_label();
    }
}
