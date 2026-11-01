use std::{
    ops::{Deref, Range},
    sync::{Arc, atomic::Ordering},
};

use ash::vk;
use deimos_data::tfx::{FixedFunctionState, PrimitiveType, ShaderStage};
use tiger_pkg::TagHash;

use crate::{
    gpu::{alloc::upload::UploadHeap, command_buffer::CommandBuffer},
    tfx::externs::LocalExternContainer,
};

use super::Gpu;

pub struct CommandList {
    gpu: Arc<Gpu>,
    pub cmd: CommandBuffer,

    state: CommandListState,
    pub(super) current_blend_state: usize,
    pub(super) current_depth_state: usize,
    pub(super) current_rasterizer_state: usize,
    pub(super) current_depth_bias: usize,
    pub(super) current_input_layout: usize,
    // pub(super) current_input_topology: usize,
    pub(super) current_stencil_ref: u32,
    pub(super) current_topology: vk::PrimitiveTopology,
    pub(super) bound_technique: TagHash,
    smart_rebind: bool,

    descriptor_buffer_bound: bool,
    current_color_formats: Vec<vk::Format>,
    current_depth_format: Option<vk::Format>,

    pub externs: LocalExternContainer,

    tag: Option<u64>,
}

impl Deref for CommandList {
    type Target = CommandBuffer;
    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

impl CommandList {
    // pub fn new(gpu: &Arc<Gpu>) -> Self {
    //     Self::from_native_command_list(
    //         gpu,
    //         NativeCommandList::new(&gpu.device).expect("Failed to create command list"),
    //     )
    // }

    pub fn from_native_command_list(gpu: &Arc<Gpu>, cmd: CommandBuffer) -> Self {
        Self {
            externs: LocalExternContainer::new(gpu.extern_source.read().clone()),
            gpu: gpu.clone(),
            cmd,
            state: CommandListState::default(),

            current_blend_state: usize::MAX,
            current_depth_state: usize::MAX,
            current_input_layout: usize::MAX,
            current_rasterizer_state: usize::MAX,
            current_depth_bias: usize::MAX,
            current_topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            current_stencil_ref: 0,
            smart_rebind: false,
            bound_technique: TagHash::NONE,

            descriptor_buffer_bound: false,
            current_color_formats: Vec::new(),
            current_depth_format: None,

            tag: None,
        }
    }

    pub const fn with_tag(mut self, tag: u64) -> Self {
        self.tag = Some(tag);
        self
    }

    pub const fn tag(&self) -> Option<u64> {
        self.tag
    }

    pub fn into_inner(self) -> CommandBuffer {
        self.cmd
    }

    // pub fn new_sublist(&self) -> Self {
    //     let mut new = CommandList::from_device_context(
    //         &self.parent,
    //         self.parent
    //             .create_deferred_context()
    //             .expect("Failed to create deferred context"),
    //     );

    //     new.state = self.state;
    //     new.state_override = self.state_override;
    //     new.depth_mode = self.depth_mode;

    //     new
    // }

    pub const fn gpu(&self) -> &Arc<Gpu> {
        &self.gpu
    }

    // pub fn upload_ring(&self) -> &UploadRing {
    //     &self.gpu.frame().upload
    // }
}

// GPU state management
impl CommandList {
    const fn reset_states(&mut self) {
        // Reset current states
        self.current_blend_state = usize::MAX;
        self.current_depth_state = usize::MAX;
        // self.current_input_layout = usize::MAX;
        self.current_rasterizer_state = usize::MAX;
        self.current_depth_bias = usize::MAX;
        // self.current_input_topology = usize::MAX;
        self.bound_technique = TagHash::NONE;
    }

    pub const fn flush_states(&mut self) {
        self.reset_states();
        if let Some(blend) = self.state.ffstate.blend_state() {
            self.set_blend_state(blend);
        }
        if let Some(depth_stencil) = self.state.ffstate.depth_stencil_state() {
            self.set_depth_stencil_state(depth_stencil);
        }
        if let Some(rasterizer) = self.state.ffstate.rasterizer_state() {
            self.set_rasterizer_state(rasterizer);
        }
        if let Some(depth_bias) = self.state.ffstate.depth_bias_state() {
            self.set_depth_bias(depth_bias);
        }
    }

    pub const fn set_blend_state(&mut self, index: usize) {
        self.current_blend_state = index;
    }

    pub fn set_depth_mode(&mut self, mode: DepthMode) {
        if self.state.depth_mode != mode {
            self.state.depth_mode = mode;
            let mut d = usize::MAX;
            std::mem::swap(&mut d, &mut self.current_depth_state);
            self.set_depth_stencil_state(d);
        }
    }

    pub const fn depth_mode(&self) -> DepthMode {
        self.state.depth_mode
    }

    pub const fn set_depth_stencil_state(&mut self, index: usize) {
        self.current_depth_state = index;
    }

    pub fn set_stencil_ref(&mut self, ref_value: u32) {
        if self.current_stencil_ref != ref_value {
            self.current_stencil_ref = ref_value;
            unsafe {
                self.gpu.cmd_set_stencil_reference(
                    *self.cmd,
                    vk::StencilFaceFlags::FRONT_AND_BACK,
                    ref_value,
                );
            }
            let d = self.current_depth_state;
            self.current_depth_state = usize::MAX;
            self.set_depth_stencil_state(d);
        }
    }

    pub const fn set_rasterizer_state(&mut self, index: usize) {
        self.current_rasterizer_state = index;
    }

    pub const fn set_depth_bias(&mut self, index: usize) {
        self.current_depth_bias = index;
    }

    pub const fn set_input_layout(&mut self, index: usize) {
        self.current_input_layout = index;
    }

    pub const fn get_input_layout(&self) -> usize {
        self.current_input_layout
    }

    pub fn bind_descriptor_buffer(&mut self) {
        if self.descriptor_buffer_bound {
            return;
        }

        let binding_info = self.gpu.frame().descriptors.binding_info();
        unsafe {
            self.gpu
                .descriptor_buffer
                .cmd_bind_descriptor_buffers(*self.cmd, std::slice::from_ref(&binding_info));
        }
        self.descriptor_buffer_bound = true;
    }

    pub fn set_descriptor_buffer_offsets(
        &self,
        pipeline_layout: vk::PipelineLayout,
        first_set: u32,
        layouts: &[vk::DescriptorSetLayout],
        offsets: &[vk::DeviceSize],
    ) {
        debug_assert_eq!(layouts.len(), offsets.len());
        debug_assert!(layouts.len() <= 8);

        let buffer_indices = [0u32; 8];
        unsafe {
            self.gpu
                .descriptor_buffer
                .cmd_set_descriptor_buffer_offsets(
                    *self.cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    first_set,
                    &buffer_indices[..layouts.len()],
                    offsets,
                );
        }
    }

    pub fn bind_pipeline(&self, pipeline: vk::Pipeline) {
        unsafe {
            self.gpu
                .cmd_bind_pipeline(*self.cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            self.gpu.cmd_set_stencil_reference(
                *self.cmd,
                vk::StencilFaceFlags::FRONT_AND_BACK,
                self.current_stencil_ref,
            );
        }
    }

    /// Applies a one-time fixed function state override
    pub const fn apply_ffstate(&mut self, states: &FixedFunctionState) {
        if let Some(u) = states.blend_state() {
            self.set_blend_state(u);
        }
        if let Some(u) = states.depth_stencil_state() {
            self.set_depth_stencil_state(u);
        }
        if let Some(u) = states.rasterizer_state() {
            self.set_rasterizer_state(u);
        }
        if let Some(u) = states.depth_bias_state() {
            self.set_depth_bias(u);
        }
    }

    pub fn set_input_topology(&mut self, topology: PrimitiveType) {
        let topology = match topology {
            PrimitiveType::PointList => vk::PrimitiveTopology::POINT_LIST,
            PrimitiveType::LineList => vk::PrimitiveTopology::LINE_LIST,
            PrimitiveType::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
            PrimitiveType::Triangles => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveType::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
        };
        self.current_topology = topology;
        self.cmd.set_primitive_topology(topology);
    }

    #[deprecated(note = "This method bypasses the pipeline state, use set_input_topology instead")]
    pub fn input_assembler_set_primitive_topology(&self, topology: vk::PrimitiveTopology) {
        self.cmd.set_primitive_topology(topology);
    }

    pub fn draw_instanced(&self, vertices: Range<u32>, instances: Range<u32>) {
        self.gpu().num_drawcalls.fetch_add(1, Ordering::Relaxed);
        self.cmd.draw(
            vertices.len() as u32,
            instances.len() as u32,
            vertices.start,
            instances.start,
        );
    }

    pub fn draw_indexed_instanced(
        &self,
        indices: Range<u32>,
        instances: Range<u32>,
        base_vertex_location: i32,
    ) {
        self.gpu().num_drawcalls.fetch_add(1, Ordering::Relaxed);
        self.cmd.draw_indexed(
            indices.len() as u32,
            instances.len() as u32,
            indices.start,
            base_vertex_location,
            instances.start,
        );
    }

    pub fn set_scissor_rects(&mut self, rects: &[vk::Rect2D]) {
        // self.state.output.scissor_rects = rects.iter().cloned().collect();
        self.cmd.set_scissor(0, rects);
    }

    pub fn set_viewports(&mut self, viewports: &[vk::Viewport]) {
        let mut viewports = viewports.to_vec();
        // Flip D3D viewports to Vulkan
        for v in &mut viewports {
            v.y += v.height;
            v.height = -v.height;
        }
        // self.state.output.viewports = viewports.iter().cloned().collect();
        self.cmd.set_viewport(0, &viewports);
    }

    pub const fn resources(&self, stage: ShaderStage) -> &StageResources {
        &self.state.resources[stage as usize - 1]
    }

    pub const fn resources_mut(&mut self, stage: ShaderStage) -> &mut StageResources {
        &mut self.state.resources[stage as usize - 1]
    }

    pub fn get_shader_resource_view(
        &mut self,
        stage: ShaderStage,
        slot: u32,
    ) -> Option<vk::ImageView> {
        self.resources(stage)
            .srvs
            .get(slot as usize)
            .cloned()
            .flatten()
    }

    pub fn get_shader_constant_buffer_view(
        &mut self,
        stage: ShaderStage,
        slot: u32,
    ) -> Option<ConstantBufferView> {
        self.resources(stage)
            .cbvs
            .get(slot as usize)
            .cloned()
            .flatten()
    }

    pub fn set_shader_resource_view(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        view: Option<vk::ImageView>,
    ) {
        if let Some(slot_mut) = self.resources_mut(stage).srvs.get_mut(slot as usize) {
            *slot_mut = view;
        }
    }

    pub fn set_shader_constant_buffer_view(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        mut cbv: Option<ConstantBufferView>,
    ) {
        if cbv.as_ref().is_some_and(|v| v.addr == 0) {
            cbv = None;
        }

        if let Some(slot_mut) = self.resources_mut(stage).cbvs.get_mut(slot as usize) {
            *slot_mut = cbv;
        }
    }

    pub fn set_shader_sampler(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        sampler: Option<vk::Sampler>,
    ) {
        if let Some(slot_mut) = self.resources_mut(stage).samplers.get_mut(slot as usize) {
            *slot_mut = sampler;
        }
    }

    pub fn set_unordered_access_view(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        view: Option<vk::ImageView>,
    ) {
        if let Some(slot_mut) = self.resources_mut(stage).uavs.get_mut(slot as usize) {
            *slot_mut = view;
        }
    }

    pub fn set_shader_resource_buffer(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        buffer: Option<BufferResource>,
    ) {
        if let Some(slot_mut) = self.resources_mut(stage).srv_buffers.get_mut(slot as usize) {
            *slot_mut = buffer;
        }
    }

    pub fn set_unordered_access_buffer(
        &mut self,
        stage: ShaderStage,
        slot: u32,
        buffer: Option<BufferResource>,
    ) {
        if let Some(slot_mut) = self.resources_mut(stage).uav_buffers.get_mut(slot as usize) {
            *slot_mut = buffer;
        }
    }

    /// Sets the attachment formats for the current rendering pass
    ///
    /// These are compiled into the graphics pipeline, techniques need to know them before creating a pipeline
    pub fn set_render_target_formats(
        &mut self,
        color_formats: &[vk::Format],
        depth_format: Option<vk::Format>,
    ) {
        self.current_color_formats.clear();
        self.current_color_formats.extend_from_slice(color_formats);
        self.current_depth_format = depth_format;
    }

    pub const fn color_attachment_formats(&self) -> &[vk::Format] {
        self.current_color_formats.as_slice()
    }

    pub const fn depth_attachment_format(&self) -> Option<vk::Format> {
        self.current_depth_format
    }

    pub const fn current_topology(&self) -> vk::PrimitiveTopology {
        self.current_topology
    }

    /// Rebinds techniques even if they are already bound
    pub const fn disable_smart_technique_binding(&mut self) {
        self.smart_rebind = false;
    }

    /// Skips rebinding techniques that are already bound
    pub const fn enable_smart_technique_binding(&mut self) {
        self.smart_rebind = true;
    }

    /// Returns true if the given technique is already bound and should not be rebound
    pub fn is_technique_smart_bound(&mut self, index: TagHash) -> bool {
        if self.bound_technique != index {
            self.bound_technique = index;
            false
        } else {
            self.smart_rebind
        }
    }

    #[profiling::function]
    pub fn restore_cmd_state(&mut self, new_state: &CommandListState) {
        // let om = &new_state.output;
        // self.cmd.om_set_render_targets(&om.rtvs, false, om.dsv);
        // self.cmd.set_viewports(&om.viewports);
        // self.cmd.set_scissor_rects(&om.scissor_rects);

        self.state = new_state.clone();
    }

    pub const fn cmd_state(&self) -> &CommandListState {
        &self.state
    }

    pub const fn set_ffstate(&mut self, ffstate: FixedFunctionState) {
        self.state.ffstate = ffstate;
    }

    pub const fn set_ffstate_override(&mut self, ffstate: FixedFunctionState) {
        self.state.ffstate_override = ffstate;
    }

    pub fn reset_ffstate_override(&mut self) {
        self.state.ffstate_override = FixedFunctionState::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthMode {
    /// Commonly used for shadow maps and decals
    Forward,
    /// Used by default
    Reverse,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ConstantBufferView {
    pub addr: vk::DeviceAddress,
    pub size: u64,
}

/// A buffer-backed resource (uniform/storage texel buffer or storage buffer).
///
/// Descriptor buffers represent texel buffers
#[derive(Debug, Clone, Copy)]
pub struct BufferResource {
    pub addr: vk::DeviceAddress,
    pub size: u64,
    pub format: vk::Format,
}

#[derive(Default, Clone)]
pub struct StageResources {
    pub srvs: [Option<vk::ImageView>; 32],
    pub cbvs: [Option<ConstantBufferView>; 32],
    pub samplers: [Option<vk::Sampler>; 32],
    pub uavs: [Option<vk::ImageView>; 32],
    pub srv_buffers: [Option<BufferResource>; 32],
    pub uav_buffers: [Option<BufferResource>; 32],
}

impl StageResources {
    pub fn get_shader_resource_view(&self, slot: u32) -> Option<vk::ImageView> {
        self.srvs.get(slot as usize).cloned().flatten()
    }

    pub fn get_shader_constant_buffer_view(&self, slot: u32) -> Option<ConstantBufferView> {
        let view = self.cbvs.get(slot as usize).cloned().flatten()?;
        if view.addr == 0 { None } else { Some(view) }
    }

    pub fn get_sampler(&self, slot: u32) -> Option<vk::Sampler> {
        self.samplers.get(slot as usize).cloned().flatten()
    }

    pub fn get_unordered_access_view(&self, slot: u32) -> Option<vk::ImageView> {
        self.uavs.get(slot as usize).cloned().flatten()
    }

    pub fn get_srv_buffer(&self, slot: u32) -> Option<BufferResource> {
        self.srv_buffers.get(slot as usize).cloned().flatten()
    }

    pub fn get_uav_buffer(&self, slot: u32) -> Option<BufferResource> {
        self.uav_buffers.get(slot as usize).cloned().flatten()
    }
}

#[macro_export]
macro_rules! cmd_event_span {
    ($cmd:ident, $name:expr) => {
        profiling::scope!(&format!("cmd-{}", $name));
        let _gpu_span = $cmd.begin_event_span($name);
    };
}

#[derive(Clone)]
pub struct CommandListState {
    pub ffstate: FixedFunctionState,
    pub ffstate_override: FixedFunctionState,
    pub(super) depth_mode: DepthMode,

    resources: [StageResources; 6],
    // pub(crate) resources_vs: StageResources,
    // pub(crate) resources_ps: StageResources,
    // pub(crate) resources_cs: StageResources,
    // pub(crate) resources_ds: StageResources,
    // pub(crate) resources_hs: StageResources,
    // pub(crate) resources_gs: StageResources,
}

impl Default for CommandListState {
    fn default() -> Self {
        Self {
            ffstate: FixedFunctionState::default(),
            ffstate_override: FixedFunctionState::default(),

            depth_mode: DepthMode::Reverse,

            resources: std::array::from_fn(|_| StageResources::default()),
            // resources_vs: StageResources::default(),
            // resources_ps: StageResources::default(),
            // resources_cs: StageResources::default(),
            // resources_ds: StageResources::default(),
            // resources_hs: StageResources::default(),
            // resources_gs: StageResources::default(),
        }
    }
}
