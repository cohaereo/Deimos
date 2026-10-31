use ahash::HashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::texture::DxgiFormat;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::asset::texture::{Texture, TextureDesc};
use crate::gpu::Gpu;

fn texture_from_hash(hash: u64) -> [u8; 32 * 32 * 4] {
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&hash.to_le_bytes());
    seed[8..16].copy_from_slice(&(!hash).to_le_bytes());
    seed[16..24].copy_from_slice(&(hash.rotate_left(17)).to_le_bytes());
    seed[24..32].copy_from_slice(&(hash.rotate_right(11)).to_le_bytes());

    let mut rng = ChaCha8Rng::from_seed(seed);

    let mut texture_small = [0u8; 4 * 4 * 4];
    rng.fill_bytes(&mut texture_small);

    let mut texture = [0u8; 32 * 32 * 4];
    for y in 0..32 {
        let small_y = y / 8;
        for x in 0..32 {
            let small_x = x / 8;

            let src = &texture_small[(small_y * 4 + small_x) * 4..];
            let dst = &mut texture[(y * 32 + x) * 4..];

            dst[0..4].copy_from_slice(&src[0..4]);
        }
    }

    texture
}

pub struct MagicTextureContainer {
    textures: RwLock<HashMap<u64, Texture>>,
}

impl MagicTextureContainer {
    pub fn get_texture(
        &self,
        gpu: &Arc<Gpu>,
        key: String,
        filler: Option<[u8; 4]>,
    ) -> anyhow::Result<vk::ImageView> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let textures = self.textures.upgradable_read();
        if let Some(tex) = textures.get(&hash) {
            Ok(tex.view)
        } else {
            let mut data = texture_from_hash(hash);
            if let Some(filler) = filler {
                for chunk in data.chunks_exact_mut(4) {
                    chunk.copy_from_slice(&filler);
                }
            }
            let texture = Texture::load(
                gpu,
                &TextureDesc::texture_2d(
                    format!("unresolved {key}"),
                    DxgiFormat::R8G8B8A8_UNORM,
                    32,
                    32,
                ),
                &data,
            )
            .context("uploading texture")?;
            let srv = texture.view;

            let mut textures = RwLockUpgradableReadGuard::upgrade(textures);
            textures.insert(hash, texture);

            Ok(srv)
        }
    }
}

impl Default for MagicTextureContainer {
    fn default() -> Self {
        Self {
            textures: RwLock::new(HashMap::default()),
        }
    }
}
