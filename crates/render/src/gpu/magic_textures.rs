use ahash::HashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{ExternIndex, texture::DxgiFormat};
use parking_lot::RwLock;
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
    textures: RwLock<HashMap<(ExternIndex, u32), Texture>>,
}

impl MagicTextureContainer {
    pub fn get_texture(
        &self,
        gpu: &Arc<Gpu>,
        key: (ExternIndex, u32),
        filler: Option<([u8; 4], u32, u32, u32)>,
    ) -> anyhow::Result<vk::ImageView> {
        if let Some(texture) = self.textures.read().get(&key) {
            return Ok(texture.view);
        }

        let name = format!("{:?}->_0x{:X}", key.0, key.1);
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let texture = if let Some((filler, width, height, depth)) = filler {
            let mut data: Vec<u8> = vec![0u8; (width * height * depth * 4) as usize];
            for chunk in data.as_chunks_mut::<4>().0 {
                chunk.copy_from_slice(&filler);
            }

            if depth > 1 {
                Texture::load(
                    gpu,
                    &TextureDesc::texture_3d(
                        format!("[PH] unresolved {name}"),
                        DxgiFormat::R8G8B8A8_UNORM,
                        width,
                        height,
                        depth,
                    ),
                    &data,
                )
                .context("uploading texture")?
            } else {
                Texture::load(
                    gpu,
                    &TextureDesc::texture_2d(
                        format!("[PH] unresolved {name}"),
                        DxgiFormat::R8G8B8A8_UNORM,
                        width,
                        height,
                    ),
                    &data,
                )
                .context("uploading texture")?
            }
        } else {
            let data = texture_from_hash(hash);
            Texture::load(
                gpu,
                &TextureDesc::texture_2d(
                    format!("[PH] unresolved {name}"),
                    DxgiFormat::R8G8B8A8_UNORM,
                    32,
                    32,
                ),
                &data,
            )
            .context("uploading texture")?
        };
        let srv = texture.view;

        self.textures.write().insert(key, texture);

        Ok(srv)
    }
}

impl Default for MagicTextureContainer {
    fn default() -> Self {
        Self {
            textures: RwLock::new(HashMap::default()),
        }
    }
}
