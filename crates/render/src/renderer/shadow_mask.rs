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

const VERTEX_SHADER: TagHash = TagHash(0x87000007);
const PIXEL_SHADER: TagHash = TagHash(0x87000008);

const VERTEX_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/generate_shadow_mask.vs.dxil");
const PIXEL_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/generate_shadow_mask.ps.dxil");

pub struct ShadowMaskRenderer {
    pass: FullscreenPass,
}

impl ShadowMaskRenderer {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        Ok(Self {
            pass: FullscreenPass::new(
                gpu,
                VERTEX_SHADER,
                PIXEL_SHADER,
                VERTEX_BYTECODE,
                PIXEL_BYTECODE,
            )?,
        })
    }
}

impl Renderer {
    pub fn generate_shadow_mask(&self, cmd: &mut CommandList, cascades: &[(Mat4, vk::ImageView)]) {
        let Some(deferred_depth) = self.externs.deferred.deferred_depth.get_srv() else {
            return;
        };

        if let Err(e) = self.generate_shadow_mask_inner(cmd, deferred_depth, cascades) {
            error!("Failed to generate shadow mask: {e:?}");
        }
    }

    fn generate_shadow_mask_inner(
        &self,
        cmd: &mut CommandList,
        deferred_depth: vk::ImageView,
        cascades: &[(Mat4, vk::ImageView)],
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        self.globals.scopes.view.bind(cmd);

        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);

        let fixed_function_state = FixedFunctionState::new(Some(0), Some(0), Some(1), Some(1));
        cmd.set_ffstate(fixed_function_state);
        cmd.flush_states();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(deferred_depth));
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

        let pass = &self.internal.shadow_mask_renderer.pass;
        let pipeline = pass.get_pipeline(&gpu, cmd, fixed_function_state)?;
        pass.bind(&gpu, cmd, pipeline)?;

        cmd.draw_instanced(0..4, 0..1);

        Ok(())
    }
}
