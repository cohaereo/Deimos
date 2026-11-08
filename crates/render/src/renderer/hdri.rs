use std::sync::Arc;

use ash::vk;
use deimos_data::tfx::{FixedFunctionState, PrimitiveType, ShaderStage};
use glam::Mat4;
use tiger_pkg::TagHash;

use crate::{
    gpu::{
        Gpu,
        command_list::{CommandList, ConstantBufferView},
    },
    renderer::{Renderer, fullscreen_pass::FullscreenPass},
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
    background: FullscreenPass,
    lighting: FullscreenPass,
}

impl HdriRenderer {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        Ok(Self {
            background: FullscreenPass::new(
                gpu,
                BACKGROUND_VERTEX_SHADER,
                BACKGROUND_PIXEL_SHADER,
                BACKGROUND_VERTEX_BYTECODE,
                BACKGROUND_PIXEL_BYTECODE,
            )?,
            lighting: FullscreenPass::new(
                gpu,
                LIGHTING_VERTEX_SHADER,
                LIGHTING_PIXEL_SHADER,
                LIGHTING_VERTEX_BYTECODE,
                LIGHTING_PIXEL_BYTECODE,
            )?,
        })
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

        cmd.draw(0..4, 0..1);

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

        cmd.draw(0..4, 0..1);

        Ok(())
    }
}
