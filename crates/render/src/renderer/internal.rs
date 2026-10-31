use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use deimos_data::tfx::texture::DxgiFormat;

use crate::{
    asset::texture::{Texture, TextureDesc},
    gpu::Gpu,
};

pub struct InternalResources {
    // pub rs_hdri_lighting: d3d12::RootSignature,
    // pub pso_hdri_lighting: d3d12::PipelineState,
    // pub rs_hdri_background: d3d12::RootSignature,
    // pub pso_hdri_background: d3d12::PipelineState,
    // pub rs_generate_shadow_mask: d3d12::RootSignature,
    // pub pso_generate_shadow_mask: d3d12::PipelineState,
    pub hdri: Texture,
    pub default_hemisphere: Texture,
}

impl InternalResources {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        info!("Loading Internal Resources");
        // let root_signature_raw = RootSignatureBuilder::default()
        //     .with_param(
        //         d3d12::RootParameter::DescriptorTable(&[DescriptorRange {
        //             base_shader_register: 0,
        //             register_space: 0,
        //             num_descriptors: 2,
        //             range_type: d3d12::DescriptorRangeType::Srv,
        //             offset_in_descriptors_from_table_start: 0,
        //         }]),
        //         d3d12::ShaderVisibility::Pixel,
        //     )
        //     .with_param(
        //         d3d12::RootParameter::CbvDescriptor {
        //             shader_register: 12,
        //             register_space: 0,
        //         },
        //         d3d12::ShaderVisibility::All,
        //     )
        //     .with_sampler(d3d12::StaticSamplerDesc {
        //         filter: d3d12::Filter::MinMagMipLinear,
        //         address_u: d3d12::TextureAddressMode::Clamp,
        //         address_v: d3d12::TextureAddressMode::Clamp,
        //         address_w: d3d12::TextureAddressMode::Clamp,
        //         mip_lod_bias: 0.0,
        //         max_anisotropy: 1,
        //         comparison_func: d3d12::ComparisonFunc::Always,
        //         border_color: d3d12::D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        //         min_lod: 0.0,
        //         max_lod: f32::MAX,
        //         shader_register: 0,
        //         register_space: 0,
        //         shader_visibility: d3d12::ShaderVisibility::Pixel,
        //     })
        //     .serialize()?;

        // let rs_hdri_background = gpu
        //     .create_root_signature(&root_signature_raw)
        //     .context("rs_hdri_background")?;
        // let pso_hdri_background = gpu
        //     .create_graphics_pipeline_state(
        //         &GraphicsPipelineStateDesc::new(&rs_hdri_background)
        //             .with_vs(include_bytes!(
        //                 "../../builtin/shaders/apply_hdri_as_background.vs.dxil"
        //             ))
        //             .with_ps(include_bytes!(
        //                 "../../builtin/shaders/apply_hdri_as_background.ps.dxil"
        //             ))
        //             .with_rtv_formats(&[Format::R11g11b10Float]), // .with_blend_state(blend_desc),
        //     )
        //     .context("pso_hdri_background")?;

        // let blend_desc = d3d12::BlendDesc::from_single_target(
        //     d3d12::RenderTargetBlendDesc::builder()
        //         .blend_enable(true)
        //         .src_blend(d3d12::Blend::One)
        //         .dest_blend(d3d12::Blend::One)
        //         .blend_op(d3d12::BlendOp::Add)
        //         .src_blend_alpha(d3d12::Blend::One)
        //         .dest_blend_alpha(d3d12::Blend::One)
        //         .blend_op_alpha(d3d12::BlendOp::Add)
        //         .render_target_write_mask(0xF)
        //         .logic_op(d3d12::LogicOp::Noop)
        //         .logic_op_enable(false)
        //         .build(),
        // );

        // let root_signature_raw = RootSignatureBuilder::default()
        //     .with_param(
        //         d3d12::RootParameter::DescriptorTable(&[
        //             DescriptorRange {
        //                 base_shader_register: 0,
        //                 register_space: 0,
        //                 num_descriptors: 4,
        //                 range_type: d3d12::DescriptorRangeType::Srv,
        //                 offset_in_descriptors_from_table_start: 0,
        //             },
        //             DescriptorRange {
        //                 base_shader_register: 10,
        //                 register_space: 0,
        //                 num_descriptors: 4,
        //                 range_type: d3d12::DescriptorRangeType::Srv,
        //                 offset_in_descriptors_from_table_start: 4,
        //             },
        //         ]),
        //         d3d12::ShaderVisibility::Pixel,
        //     )
        //     .with_cbv(0, 0, d3d12::ShaderVisibility::All)
        //     .with_cbv(12, 0, d3d12::ShaderVisibility::All)
        //     .with_sampler(d3d12::StaticSamplerDesc {
        //         filter: d3d12::Filter::MinMagMipLinear,
        //         address_u: d3d12::TextureAddressMode::Clamp,
        //         address_v: d3d12::TextureAddressMode::Clamp,
        //         address_w: d3d12::TextureAddressMode::Clamp,
        //         mip_lod_bias: 0.0,
        //         max_anisotropy: 1,
        //         comparison_func: d3d12::ComparisonFunc::Always,
        //         border_color: d3d12::D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        //         min_lod: 0.0,
        //         max_lod: f32::MAX,
        //         shader_register: 0,
        //         register_space: 0,
        //         shader_visibility: d3d12::ShaderVisibility::Pixel,
        //     })
        //     .serialize()?;

        // let rs_hdri_lighting = gpu
        //     .create_root_signature(&root_signature_raw)
        //     .context("rs_hdri_lighting")?;
        // let pso_hdri_lighting = gpu
        //     .create_graphics_pipeline_state(
        //         &GraphicsPipelineStateDesc::new(&rs_hdri_lighting)
        //             .with_vs(include_bytes!(
        //                 "../../builtin/shaders/apply_hdri_as_light.vs.dxil"
        //             ))
        //             .with_ps(include_bytes!(
        //                 "../../builtin/shaders/apply_hdri_as_light.ps.dxil"
        //             ))
        //             .with_rtv_formats(&[Format::R11g11b10Float, Format::R11g11b10Float])
        //             .with_blend_state(blend_desc),
        //     )
        //     .context("pso_hdri_lighting")?;

        // let root_signature_raw = RootSignatureBuilder::default()
        //     .with_param(
        //         d3d12::RootParameter::DescriptorTable(&[
        //             DescriptorRange {
        //                 base_shader_register: 0,
        //                 register_space: 0,
        //                 num_descriptors: 1,
        //                 range_type: d3d12::DescriptorRangeType::Srv,
        //                 offset_in_descriptors_from_table_start: 0,
        //             },
        //             DescriptorRange {
        //                 base_shader_register: 10,
        //                 register_space: 0,
        //                 num_descriptors: 4,
        //                 range_type: d3d12::DescriptorRangeType::Srv,
        //                 offset_in_descriptors_from_table_start: 1,
        //             },
        //         ]),
        //         d3d12::ShaderVisibility::Pixel,
        //     )
        //     .with_cbv(0, 0, d3d12::ShaderVisibility::All)
        //     .with_cbv(12, 0, d3d12::ShaderVisibility::All)
        //     .with_sampler(d3d12::StaticSamplerDesc {
        //         filter: d3d12::Filter::MinMagMipLinear,
        //         address_u: d3d12::TextureAddressMode::Clamp,
        //         address_v: d3d12::TextureAddressMode::Clamp,
        //         address_w: d3d12::TextureAddressMode::Clamp,
        //         mip_lod_bias: 0.0,
        //         max_anisotropy: 1,
        //         comparison_func: d3d12::ComparisonFunc::Always,
        //         border_color: d3d12::D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        //         min_lod: 0.0,
        //         max_lod: f32::MAX,
        //         shader_register: 0,
        //         register_space: 0,
        //         shader_visibility: d3d12::ShaderVisibility::Pixel,
        //     })
        //     .serialize()?;

        // let rs_generate_shadow_mask = gpu
        //     .create_root_signature(&root_signature_raw)
        //     .context("rs_generate_shadow_mask")?;
        // let pso_generate_shadow_mask = gpu
        //     .create_graphics_pipeline_state(
        //         &GraphicsPipelineStateDesc::new(&rs_generate_shadow_mask)
        //             .with_vs(include_bytes!(
        //                 "../../builtin/shaders/generate_shadow_mask.vs.dxil"
        //             ))
        //             .with_ps(include_bytes!(
        //                 "../../builtin/shaders/generate_shadow_mask.ps.dxil"
        //             ))
        //             .with_rtv_formats(&[Format::R8g8Unorm]),
        //     )
        //     .context("pso_generate_shadow_mask")?;

        const HDRI_DATA_ZSTD: &[u8] = include_bytes!("../../builtin/textures/hdri_mips.data.zst");
        let hdri_mips = zstd::decode_all(HDRI_DATA_ZSTD)?;
        let num_mips = 512u16.ilog2() as u16 + 1;
        // let hdri_mips = generate_hdri_mips_rgbaf32_raw(&hdri_raw, 1024, 512);
        // std::fs::write("hdri_mips.bin", &hdri_mips)?;
        let hdri = Texture::load(
            gpu,
            &TextureDesc {
                num_mips,
                ..TextureDesc::texture_2d("hdri", DxgiFormat::R32G32B32A32_FLOAT, 1024, 512)
            },
            &hdri_mips,
        )
        .context("hdri")?;

        let hemisphere_data_l8 = include_bytes!("../../builtin/textures/default_hemisphere.data");
        let hemisphere_data_rgba8 = (0..(512 * 512))
            .flat_map(|i| {
                let l = hemisphere_data_l8[i];
                [l, l, l, 255]
            })
            .collect::<Vec<u8>>();

        let default_hemisphere = Texture::load(
            gpu,
            &TextureDesc::texture_2d("default_hemisphere", DxgiFormat::R8G8B8A8_UNORM, 512, 512),
            &hemisphere_data_rgba8,
        )
        .context("default_hemisphere")?;

        Ok(Self {
            // rs_hdri_lighting,
            // pso_hdri_lighting,

            // rs_hdri_background,
            // pso_hdri_background,

            // rs_generate_shadow_mask,
            // pso_generate_shadow_mask,
            hdri,
            default_hemisphere,
        })
    }
}
