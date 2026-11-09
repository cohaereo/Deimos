use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{FixedFunctionState, PrimitiveType, ShaderStage};
use glam::{Mat4, Vec4};
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowMaskConstants {
    world_to_cascade_projective: [Mat4; 4],
    cascade_splits: [f32; 4],
    cascade_texel_bias: [f32; 4],
    sun_direction: Vec4,
}

pub struct ShadowMaskRenderer {
    pass: FullscreenPass,
    point_sampler: vk::Sampler,
}

impl ShadowMaskRenderer {
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
    pub fn generate_shadow_mask(
        &self,
        cmd: &mut CommandList,
        cascades: &[(Mat4, vk::ImageView)],
        cascade_splits: [f32; 4],
        cascade_texel_bias: [f32; 4],
    ) {
        assert_eq!(cascades.len(), 4);

        let Some(deferred_depth) = self.externs.deferred.deferred_depth.get_srv() else {
            return;
        };
        let Some(gbuffer_normal) = self.externs.deferred.deferred_rt1.get_srv() else {
            return;
        };

        if let Err(e) = self.generate_shadow_mask_inner(
            cmd,
            deferred_depth,
            gbuffer_normal,
            cascades,
            cascade_splits,
            cascade_texel_bias,
        ) {
            error!("Failed to generate shadow mask: {e:?}");
        }
    }

    fn generate_shadow_mask_inner(
        &self,
        cmd: &mut CommandList,
        deferred_depth: vk::ImageView,
        gbuffer_normal: vk::ImageView,
        cascades: &[(Mat4, vk::ImageView)],
        cascade_splits: [f32; 4],
        cascade_texel_bias: [f32; 4],
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        self.globals.scopes.view.bind(cmd);

        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);

        let fixed_function_state = FixedFunctionState::new(Some(0), Some(0), Some(1), Some(1));
        cmd.set_ffstate(fixed_function_state);
        cmd.flush_states();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(deferred_depth));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 1, Some(gbuffer_normal));
        for (i, (_, view)) in cascades.iter().enumerate() {
            cmd.set_shader_resource_view(ShaderStage::Pixel, 10 + i as u32, Some(*view));
        }
        cmd.set_shader_sampler(
            ShaderStage::Pixel,
            0,
            Some(self.internal.shadow_mask_renderer.point_sampler),
        );

        let mut constants = ShadowMaskConstants {
            world_to_cascade_projective: [Mat4::IDENTITY; 4],
            cascade_splits,
            cascade_texel_bias,
            sun_direction: self
                .externs
                .get_global_channel_by_name("sun_light_direction"),
        };
        for (dst, (matrix, _)) in constants
            .world_to_cascade_projective
            .iter_mut()
            .zip(cascades)
        {
            *dst = *matrix;
        }

        let constants = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&constants))
            .context("Failed to upload shadow mask constants (UploadHeap OOM)")?;

        cmd.set_shader_constant_buffer_view(
            ShaderStage::Pixel,
            0,
            Some(ConstantBufferView {
                addr: constants.virtual_address(),
                size: constants.len() as u64,
            }),
        );

        let pass = &self.internal.shadow_mask_renderer.pass;
        let pipeline = pass.get_pipeline(&gpu, cmd, fixed_function_state)?;
        pass.bind(&gpu, cmd, pipeline)?;

        cmd.draw(0..4, 0..1);

        Ok(())
    }
}
