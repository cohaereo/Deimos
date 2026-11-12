use glam::Vec4;
use tiger_parse::{tiger_type, NullString, Pointer};
use tiger_pkg::TagHash;

use crate::tag::TagRef;

#[tiger_type(id = 0x8080B61C)]
pub struct SRenderGlobals {
    pub file_size: u64,
    pub unk8: Vec<SUnk8080870f>,
    pub unk18: Vec<()>,
}

#[tiger_type(id = 0x8080A7C9)]
pub struct SUnk8080870f {
    pub unk0: u32,
    pub unk4: u32,
    pub unk8: TagRef<SRenderGlobalsData>,
    pub unkc: u32,
}

#[tiger_type(id = 0x80808070)]
pub struct SRenderGlobalsData {
    pub file_size: u64,
    pub input_layouts: TagRef<SVertexInputLayouts>,
    _padc: u32,
    pub scopes: Vec<SRenderGlobalScope>,
    pub pipelines: Vec<SRenderGlobalPipelines>,
    /// Lookup textures
    pub unk30: TagRef<SRenderGlobalLookupTextures>,
    pub global_channels: TagRef<SRenderGlobalsGlobalChannels>,
    pub unk5e_channels: TagRef<SRenderGlobalsUnk5eChannels>,
    pub unk3c: TagRef<SWaterDisplacementGlobals>,
    pub unk40: TagHash,
}

#[tiger_type(id = 0x8080810E)]
pub struct SWaterDisplacementGlobals {
    pub file_size: u64,
    pub water_displacement_unk00: TagHash,
    pub water_displacement_unk08: TagHash,
    pub unk10: TagHash,
    pub unk14: f32,
    pub unk18: f32,
    pub unk1c: f32,
    pub unk20: f32,
    pub unk24: f32,
    pub unk28: f32,
    pub unk2c: f32,
}

#[derive(Debug)]
#[tiger_type(id = 0x80807F6A)]
pub struct SRenderGlobalLookupTextures {
    pub file_size: u64,
    pub specular_tint_lookup_texture: TagHash,
    pub specular_lobe_lookup_texture: TagHash,
    pub specular_lobe_3d_lookup_texture: TagHash,
    pub iridescence_lookup_texture: TagHash,
}

#[derive(Debug)]
#[tiger_type(id = 0x80808075)]
pub struct SRenderGlobalScope {
    pub name: Pointer<NullString>,
    pub unk8: u32,
    // TODO(cohae): Optional Tag<T>
    pub scope: TagHash,
}

#[derive(Debug)]
#[tiger_type(id = 0x80808074)]
pub struct SRenderGlobalPipelines {
    pub name: Pointer<NullString>,
    pub unk8: u32,
    pub technique: TagHash,
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080A014)]
// cohae: I love this name
pub struct SRenderGlobalsGlobalChannels {
    pub file_size: u64,
    pub channel_ids: Vec<u32>,
    pub default_values: Vec<Vec4>,
    pub unk28: Vec<()>,
}

impl SRenderGlobalsGlobalChannels {
    pub fn default_values(&self) -> [Vec4; 256] {
        let mut default_values = [Vec4::ZERO; 256];
        default_values[..self.default_values.len()].copy_from_slice(&self.default_values);
        default_values
    }
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080A002)]
pub struct SRenderGlobalsUnk5eChannels {
    pub file_size: u64,
    pub all_channels: Vec<S8080A004>, // official name
    pub set1: Vec<S8080A004>,         // official name pending (hash 0x84E4911C)
    // TODO(cohae): Some kind of quick inline lookup?
    pub unk28: [S8080A004; 6],
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080A004)]
pub struct S8080A004 {
    pub id: u32,
    pub index: u32,
}

#[tiger_type(id = 0x80808661, size = 0x38)]
pub struct SVertexInputLayouts {
    pub file_size: u64,
    pub unk8: u32,
    pub elements_c: TagRef<SVertexInputElementSets>,
    pub elements_10: TagHash,
    pub elements_14: TagHash,
    pub elements_18: TagHash,
    pub elements_1c: TagHash,
    pub elements_20: TagHash,
    pub elements_24: TagHash,
    pub elements_28: TagHash,
    pub elements_2c: TagHash,
    pub elements_30: TagHash,
    pub mapping: TagRef<SVertexInputLayoutMapping>,
}

#[tiger_type(id = 0x80808664, size = 0x18)]
pub struct SVertexInputLayoutMapping {
    pub file_size: u64,
    pub layouts: Vec<SVertexLayout>,
}

#[derive(Debug)]
#[tiger_type(id = 0x80808667, size = 0x1c)]
pub struct SVertexLayout {
    pub index: u8,

    #[tiger(offset = 0x8)]
    pub buffer_0: u32,
    pub buffer_1: u32,
    pub buffer_2: u32,
    pub buffer_3: u32,

    pub buffer_0_instanced: bool,
    pub buffer_1_instanced: bool,
    pub buffer_2_instanced: bool,
    pub buffer_3_instanced: bool,
}

#[tiger_type(id = 0x80808668, size = 0x18)]
pub struct SVertexInputElementSets {
    pub file_size: u64,
    pub sets: Vec<SVertexInputElementSet>,
}

#[tiger_type(id = 0x8080866A, size = 0x10)]
pub struct SVertexInputElementSet {
    pub elements: Vec<SVertexInputElement>,
}

#[tiger_type(id = 0x8080866D, size = 3)]
pub struct SVertexInputElement {
    pub semantic: u8,
    pub semantic_index: u8,
    pub format: u8,
}
