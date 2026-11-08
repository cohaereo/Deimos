use std::sync::Arc;

use ash::vk;
use deimos_data::tfx::{FixedFunctionState, PrimitiveType, ShaderStage};
use glam::Mat4;
use smallvec::SmallVec;
use tiger_pkg::TagHash;

use crate::{
    gpu::{
        Gpu,
        command_list::{CommandList, ConstantBufferView},
        pipeline_cache::{PipelineKey, ShaderInfo, TechniqueLayout},
    },
    renderer::Renderer,
    tfx::technique::Technique,
};

const BACKGROUND_VERTEX_SHADER: TagHash = TagHash(0x87000003);
const BACKGROUND_PIXEL_SHADER: TagHash = TagHash(0x87000004);
const LIGHTING_VERTEX_SHADER: TagHash = TagHash(0x87000005);
const LIGHTING_PIXEL_SHADER: TagHash = TagHash(0x87000006);

const BACKGROUND_VERTEX_BYTECODE: &[u8] =
    include_bytes!("../../builtin/shaders/apply_hdri_as_background.vs.dxil");
const BACKGROUND_PIXEL_BYTECODE: &[u8] =
    include_bytes!("../../builtin/shaders/apply_hdri_as_background.ps.dxil");
const LIGHTING_VERTEX_BYTECODE: &[u8] =
    include_bytes!("../../builtin/shaders/apply_hdri_as_light.vs.dxil");
const LIGHTING_PIXEL_BYTECODE: &[u8] =
    include_bytes!("../../builtin/shaders/apply_hdri_as_light.ps.dxil");

pub struct HdriRenderer {
    background: HdriPass,
    lighting: HdriPass,
}

impl HdriRenderer {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        Ok(Self {
            background: HdriPass::new(
                gpu,
                BACKGROUND_VERTEX_SHADER,
                BACKGROUND_PIXEL_SHADER,
                BACKGROUND_VERTEX_BYTECODE,
                BACKGROUND_PIXEL_BYTECODE,
            )?,
            lighting: HdriPass::new(
                gpu,
                LIGHTING_VERTEX_SHADER,
                LIGHTING_PIXEL_SHADER,
                LIGHTING_VERTEX_BYTECODE,
                LIGHTING_PIXEL_BYTECODE,
            )?,
        })
    }
}

struct HdriPass {
    vertex_shader_tag: TagHash,
    pixel_shader_tag: TagHash,
    layout: TechniqueLayout,
    vertex_shader: ShaderInfo,
    pixel_shader: ShaderInfo,
}

impl HdriPass {
    fn new(
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

    fn get_pipeline(
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

    fn bind(
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

impl Renderer {
    pub fn draw_hdri_background(&self, cmd: &mut CommandList) {
        let Some(deferred_depth) = self.externs.deferred.deferred_depth.get_srv() else {
            return;
        };

        if let Err(e) = self.draw_hdri_background_inner(cmd, deferred_depth) {
            error!("Failed to draw HDRI background: {e:?}");
        }
    }

    fn draw_hdri_background_inner(
        &self,
        cmd: &mut CommandList,
        deferred_depth: vk::ImageView,
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);

        let fixed_function_state = FixedFunctionState::new(Some(0), Some(0), Some(1), Some(1));
        cmd.set_ffstate(fixed_function_state);
        cmd.flush_states();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(deferred_depth));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 1, Some(self.internal.hdri.view));
        cmd.set_shader_sampler(ShaderStage::Pixel, 0, Some(gpu.default_sampler));

        let pass = &self.internal.hdri_renderer.background;
        let pipeline = pass.get_pipeline(&gpu, cmd, fixed_function_state)?;
        pass.bind(&gpu, cmd, pipeline)?;

        cmd.draw_instanced(0..4, 0..1);

        Ok(())
    }

    pub fn apply_hdri_light(&self, cmd: &mut CommandList, cascades: &[(Mat4, vk::ImageView)]) {
        assert_eq!(cascades.len(), 4);

        let Some(gbuffer_normal) = self.externs.deferred.deferred_rt1.get_srv() else {
            return;
        };
        let Some(gbuffer_third) = self.externs.deferred.deferred_rt2.get_srv() else {
            return;
        };
        let Some(deferred_depth) = self.externs.deferred.deferred_depth.get_srv() else {
            return;
        };

        if let Err(e) = self.apply_hdri_light_inner(
            cmd,
            gbuffer_normal,
            gbuffer_third,
            deferred_depth,
            cascades,
        ) {
            error!("Failed to apply HDRI light: {e:?}");
        }
    }

    fn apply_hdri_light_inner(
        &self,
        cmd: &mut CommandList,
        gbuffer_normal: vk::ImageView,
        gbuffer_third: vk::ImageView,
        deferred_depth: vk::ImageView,
        cascades: &[(Mat4, vk::ImageView)],
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        self.globals.scopes.view.bind(cmd);

        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);

        let fixed_function_state = FixedFunctionState::new(Some(2), Some(0), Some(1), Some(1));
        cmd.set_ffstate(fixed_function_state);
        cmd.flush_states();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(gbuffer_normal));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 1, Some(gbuffer_third));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 2, Some(deferred_depth));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 3, Some(self.internal.hdri.view));
        for (i, (_, view)) in cascades.iter().enumerate() {
            cmd.set_shader_resource_view(ShaderStage::Pixel, 10 + i as u32, Some(*view));
        }
        cmd.set_shader_sampler(ShaderStage::Pixel, 0, Some(gpu.default_sampler));

        let cascade_matrices = cascades.iter().map(|(m, _)| *m).collect::<Vec<_>>();
        let cascade_matrices = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::cast_slice(&cascade_matrices))?;
        cmd.set_shader_constant_buffer_view(
            ShaderStage::Pixel,
            0,
            Some(ConstantBufferView {
                addr: cascade_matrices.virtual_address(),
                size: cascade_matrices.len() as u64,
            }),
        );

        let pass = &self.internal.hdri_renderer.lighting;
        let pipeline = pass.get_pipeline(&gpu, cmd, fixed_function_state)?;
        pass.bind(&gpu, cmd, pipeline)?;

        cmd.draw_instanced(0..4, 0..1);

        Ok(())
    }
}
