pub mod handle;
pub mod index_buffer;
pub mod manager;
pub mod texture;
pub mod vertex_buffer;

pub use handle::Handle;
pub use manager::AssetManager;

use crate::tfx::technique::Technique;

#[derive(Debug)]
pub enum AssetKind {
    Texture,
    VertexBuffer,
    IndexBuffer,
    Technique,
}

pub trait Asset {
    const ASSET_KIND: AssetKind;
}

impl Asset for texture::Texture {
    const ASSET_KIND: AssetKind = AssetKind::Texture;
}

impl Asset for vertex_buffer::VertexBuffer {
    const ASSET_KIND: AssetKind = AssetKind::VertexBuffer;
}

impl Asset for index_buffer::IndexBuffer {
    const ASSET_KIND: AssetKind = AssetKind::IndexBuffer;
}

impl Asset for Technique {
    const ASSET_KIND: AssetKind = AssetKind::Technique;
}
