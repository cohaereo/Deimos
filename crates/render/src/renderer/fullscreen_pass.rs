use std::sync::Arc;

use ash::vk;
use deimos_data::tfx::{FixedFunctionState, ShaderStage};
use smallvec::SmallVec;
use tiger_pkg::TagHash;

use crate::{
    gpu::{
        Gpu,
        command_list::CommandList,
        pipeline_cache::{PipelineKey, ShaderInfo, TechniqueLayout},
    },
    tfx::technique::Technique,
};

pub struct FullscreenPass {
    vertex_shader_tag: TagHash,
    pixel_shader_tag: TagHash,
    layout: TechniqueLayout,
    vertex_shader: ShaderInfo,
    pixel_shader: ShaderInfo,
}

impl FullscreenPass {
    pub fn new(
        gpu: &Arc<Gpu>,
        vertex_shader_tag: TagHash,
        pixel_shader_tag: TagHash,
        vertex_bytecode: &'static [u8],
        pixel_bytecode: &'static [u8],
    ) -> anyhow::Result<Self> {
        let (layout, vertex_shader, pixel_shader) = {
            let mut cache = gpu.pipeline_cache.write();
            cache.register_builtin_shader_module(vertex_shader_tag, vertex_bytecode);
            cache.register_builtin_shader_module(pixel_shader_tag, pixel_bytecode);

            (
                cache.get_or_create_layout(vertex_shader_tag, pixel_shader_tag)?,
                cache.get_or_load_bytecode(vertex_shader_tag, ShaderStage::Vertex)?,
                cache.get_or_load_bytecode(pixel_shader_tag, ShaderStage::Pixel)?,
            )
        };

        Ok(Self {
            vertex_shader_tag,
            pixel_shader_tag,
            layout,
            vertex_shader,
            pixel_shader,
        })
    }

    pub fn get_pipeline(
        &self,
        gpu: &Arc<Gpu>,
        cmd: &CommandList,
        fixed_function_state: FixedFunctionState,
    ) -> anyhow::Result<vk::Pipeline> {
        let key = PipelineKey {
            vertex_shader: self.vertex_shader_tag,
            pixel_shader: self.pixel_shader_tag,
            depth_mode: cmd.depth_mode(),
            fixed_function_state,
            topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
            input_layout: 0,
            color_formats: SmallVec::from_slice(cmd.color_attachment_formats()),
            depth_format: cmd.depth_attachment_format(),
        };

        gpu.pipeline_cache.write().get_or_create(&key)
    }

    pub fn bind(
        &self,
        gpu: &Arc<Gpu>,
        cmd: &mut CommandList,
        pipeline: vk::Pipeline,
    ) -> anyhow::Result<()> {
        let mut offsets = [0u64; 8];
        for set_index in 0..self.layout.set_layouts.len() {
            let size = self.layout.set_sizes[set_index];
            if size == 0 {
                continue;
            }

            let offset = gpu.frame().descriptors.alloc(size)?;
            offsets[set_index] = offset as vk::DeviceSize;

            let (shader_stage, slots) = if set_index < 4 {
                (ShaderStage::Vertex, &self.vertex_shader.slots)
            } else {
                (ShaderStage::Pixel, &self.pixel_shader.slots)
            };

            Technique::write_set(
                gpu,
                shader_stage,
                slots,
                (set_index % 4) as u32,
                &self.layout.binding_offsets[set_index],
                gpu.frame().descriptors.ptr_at(offset),
                cmd,
            );
        }

        cmd.bind_descriptor_buffer();
        cmd.set_descriptor_buffer_offsets(
            self.layout.pipeline_layout,
            0,
            &self.layout.set_layouts,
            &offsets,
        );
        cmd.bind_pipeline(pipeline);

        Ok(())
    }
}
