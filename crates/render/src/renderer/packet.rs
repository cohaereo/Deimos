use std::{ffi::c_void, fmt::Debug, ptr::NonNull};

use bytemuck::Pod;
use deimos_data::tfx::{RenderStage, features::dynamic::RenderStageSubscription};
use static_assertions::assert_eq_size;

use crate::renderer::object::RenderObjectHandle;

#[derive(Default)]
pub struct FramePacket {
    allocator: bumpalo::Bump,
    pub per_frame_nodes: Vec<RenderPerFrameNode>,
    pub views: Vec<ViewPacket>,
}

impl FramePacket {
    pub fn reset(&mut self) {
        self.views.clear();
        self.per_frame_nodes.clear();
        self.allocator.reset();
    }

    pub fn push_frame_node<T: Pod>(
        &mut self,
        object: RenderObjectHandle,
        data: Option<T>,
    ) -> usize {
        let node = RenderPerFrameNode {
            object,
            data: data.map(|d| self.allocate_data(d)),
        };

        let index = self.per_frame_nodes.len();
        self.per_frame_nodes.push(node);
        index
    }

    pub fn insert_view(&mut self, view_id: usize) {
        self.views.resize_with(view_id + 1, ViewPacket::default);
        self.views[view_id].id = view_id;
    }

    pub fn push_view_node<T: Pod>(
        &mut self,
        view_id: usize,
        frame_node: usize,
        data: Option<T>,
    ) -> usize {
        if frame_node >= self.per_frame_nodes.len() {
            error!("Frame node index {} is out of bounds", frame_node);
            return usize::MAX;
        }

        let node = RenderPerViewNode {
            frame_node,
            data: data.map(|d| self.allocate_data(d)),
        };

        let Some(view_packet) = self.views.get_mut(view_id) else {
            error!("View ID {} does not exist in the frame packet", view_id);
            return usize::MAX;
        };

        let index = view_packet.view_nodes.len();
        view_packet.view_nodes.push(node);
        index
    }

    pub fn allocate_data<T: Pod + 'static>(&mut self, data: T) -> NonNull<c_void> {
        let layout = std::alloc::Layout::new::<T>();
        let ptr = self.allocator.alloc_layout(layout).cast::<T>();
        unsafe { ptr.write(data) };
        ptr.cast::<c_void>()
    }
}

pub struct ViewPacket {
    pub id: usize,
    pub view_nodes: Vec<RenderPerViewNode>,
    pub submit_node_blocks: SubmitNodeContainer,
}

impl Default for ViewPacket {
    fn default() -> Self {
        Self {
            id: 0,
            view_nodes: Vec::with_capacity(4096),
            submit_node_blocks: SubmitNodeContainer::default(),
        }
    }
}

pub struct SubmitNodeContainer([Vec<SubmitNode>; RenderStage::COUNT]);

impl SubmitNodeContainer {
    pub fn block(&self, stage: RenderStage) -> &[SubmitNode] {
        &self.0[stage as usize]
    }

    pub const fn block_mut(&mut self, stage: RenderStage) -> &mut Vec<SubmitNode> {
        &mut self.0[stage as usize]
    }

    pub const fn blocks(&self) -> &[Vec<SubmitNode>] {
        &self.0
    }

    pub const fn blocks_mut(&mut self) -> &mut [Vec<SubmitNode>] {
        &mut self.0
    }

    pub fn push(&mut self, stage: RenderStage, node: SubmitNode) {
        self.block_mut(stage).push(node);
    }

    #[profiling::function]
    pub fn broadcast(&mut self, stages: RenderStageSubscription, node: SubmitNode) {
        for stage in stages.iter() {
            self.0[stage.bits().ilog2() as usize].push(node);
        }
    }

    /// Reserves space for additional submit nodes in the specified stage, if needed.
    pub fn ensure_capacity(&mut self, stage: RenderStage, additional: usize) {
        let block = self.block_mut(stage);
        if block.capacity() - block.len() < additional {
            block.reserve(additional);
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut r = Self(std::array::from_fn(|_| Vec::new()));

        r.ensure_capacity(RenderStage::DepthPrepass, capacity);
        r.ensure_capacity(RenderStage::GenerateGbuffer, capacity);
        r.ensure_capacity(RenderStage::ShadowGenerate, capacity);
        r.ensure_capacity(RenderStage::Transparents, capacity);

        r
    }
}

impl Default for SubmitNodeContainer {
    fn default() -> Self {
        Self::with_capacity(28_800)
    }
}

impl Debug for SubmitNodeContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SubmitNodeContainer");
        for (i, stage) in RenderStage::iter().enumerate() {
            let block = &self.0[i];
            if !block.is_empty() {
                // s.field(&format!("{stage:?}"), block);
                s.field(&format!("{stage:?}"), &format!("[{} nodes]", block.len()));
            }
        }
        s.finish()
    }
}

#[repr(Rust)]
#[derive(Debug)]
pub struct RenderPerFrameNode {
    pub object: RenderObjectHandle,
    pub data: Option<NonNull<c_void>>,
}

impl RenderPerFrameNode {
    /// # Safety
    /// Callers must ensure that the data type used is the same as/compatible with the data type in the node
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn data<T: Pod + 'static>(&self) -> Option<&mut T> {
        unsafe { self.data.map(|ptr| &mut *(ptr.as_ptr().cast::<T>())) }
    }
}

assert_eq_size!(RenderPerFrameNode, [u8; 16]);

unsafe impl Send for RenderPerFrameNode {}
unsafe impl Sync for RenderPerFrameNode {}

#[repr(Rust)]
#[derive(Debug)]
pub struct RenderPerViewNode {
    pub frame_node: usize,

    pub data: Option<NonNull<c_void>>,
}

impl RenderPerViewNode {
    /// # Safety
    /// Callers must ensure that the data type used is the same as/compatible with the data type in the node
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn data<T: Pod + 'static>(&self) -> Option<&mut T> {
        unsafe { self.data.map(|ptr| &mut *(ptr.as_ptr().cast::<T>())) }
    }
}

assert_eq_size!(RenderPerViewNode, [u8; 16]);

unsafe impl Send for RenderPerViewNode {}
unsafe impl Sync for RenderPerViewNode {}

#[repr(Rust)]
#[derive(Clone, Copy)]
pub struct SubmitNode {
    pub view_node: usize,
    pub key: u64,
}

impl Debug for SubmitNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubmitNode")
            .field("view_node", &self.view_node)
            .field("key", &format_args!("SubmitKey(0x{:016X})", self.key))
            .finish()
    }
}
