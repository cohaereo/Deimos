use std::sync::Arc;

use anyhow::Context;
use deimos_data::tfx::texture::DxgiFormat;

use crate::{
    asset::texture::{Texture, TextureDesc},
    gpu::Gpu,
    renderer::{hdri::HdriRenderer, overdraw::OverdrawRenderer, shadow_mask::ShadowMaskRenderer},
};

pub struct InternalResources {
    pub hdri: Texture,
    pub hdri_renderer: HdriRenderer,
    pub shadow_mask_renderer: ShadowMaskRenderer,
    pub overdraw: OverdrawRenderer,
    pub default_hemisphere: Texture,
    pub white: Texture,
}

impl InternalResources {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        info!("Loading Internal Resources");

        let hdri_renderer = HdriRenderer::new(gpu).context("hdri_renderer")?;
        let shadow_mask_renderer = ShadowMaskRenderer::new(gpu).context("shadow_mask_renderer")?;
        let overdraw = OverdrawRenderer::new(gpu).context("overdraw")?;

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

        let white = Texture::load(
            gpu,
            &TextureDesc::texture_2d("white", DxgiFormat::R8G8B8A8_UNORM, 1, 1),
            &[255, 255, 255, 255],
        )
        .context("white")?;

        Ok(Self {
            hdri,
            hdri_renderer,
            shadow_mask_renderer,
            overdraw,
            default_hemisphere,
            white,
        })
    }
}
