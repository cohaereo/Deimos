use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{FixedFunctionState, PrimitiveType, ShaderStage};
use tiger_pkg::TagHash;

use crate::{
    gpu::{Gpu, command_list::CommandList},
    renderer::{Renderer, fullscreen_pass::FullscreenPass},
};

const VERTEX_SHADER: TagHash = TagHash(0x87000009);
const PIXEL_SHADER: TagHash = TagHash(0x8700000a);

const VERTEX_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/overdraw.vs.dxil");
const PIXEL_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/overdraw.ps.dxil");

pub struct OverdrawRenderer {
    pass: FullscreenPass,
    point_sampler: vk::Sampler,
}

impl OverdrawRenderer {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        let point_sampler = unsafe {
            gpu.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )?
        };

        Ok(Self {
            pass: FullscreenPass::new(
                gpu,
                VERTEX_SHADER,
                PIXEL_SHADER,
                VERTEX_BYTECODE,
                PIXEL_BYTECODE,
            )?,
            point_sampler,
        })
    }
}

impl Renderer {
    pub fn draw_overdraw_overlay(
        &self,
        cmd: &mut CommandList,
        overdraw_texture: vk::ImageView,
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        self.globals.scopes.view.bind(cmd);

        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);

        let fixed_function_state = FixedFunctionState::new(Some(0), Some(0), Some(1), Some(1));
        cmd.set_ffstate(fixed_function_state);
        cmd.flush_states();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(overdraw_texture));
        cmd.set_shader_sampler(
            ShaderStage::Pixel,
            0,
            Some(self.internal.overdraw.point_sampler),
        );

        let pass = &self.internal.overdraw.pass;
        let pipeline = pass.get_pipeline(&gpu, cmd, fixed_function_state)?;
        pass.bind(&gpu, cmd, pipeline)?;

        cmd.draw(0..4, 0..1);

        Ok(())
    }
}
