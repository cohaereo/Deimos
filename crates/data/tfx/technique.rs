use std::{
    fmt::Debug,
    io::{Read, Seek},
};

use glam::Vec4;
use int_enum::IntEnum;
use tiger_parse::{tiger_type, Endian, Padding, TigerReadable, TigerReader};
use tiger_pkg::TagHash;

use crate::{tag::WideHash, tfx::enums::ShaderStage};

#[derive(Clone)]
#[tiger_type(id = 0x808031D8)]
pub struct STechnique {
    pub file_size: u64,
    pub bind_mode: TechniqueBindMode,
    pub unkc: u32,
    pub unk10: u32,
    pub unk14: u32,
    pub unk18: u32,
    pub unk1c: u32,

    pub used_scopes: TfxScopeBits,
    pub compatible_scopes: TfxScopeBits,

    pub states: FixedFunctionState,
    pub unk34: [u32; 9],

    // 0x58
    pub shader_vertex: STechniqueStage,
    pub shader_hull: STechniqueStage,
    pub shader_domain: STechniqueStage,
    pub shader_geometry: STechniqueStage,
    pub shader_pixel: STechniqueStage,
    pub shader_compute: STechniqueStage,
}

impl STechnique {
    pub fn all_shaders(&self) -> Vec<(ShaderStage, &STechniqueStage)> {
        vec![
            (ShaderStage::Vertex, &self.shader_vertex),
            (ShaderStage::Geometry, &self.shader_geometry),
            (ShaderStage::Hull, &self.shader_hull),
            (ShaderStage::Domain, &self.shader_domain),
            (ShaderStage::Pixel, &self.shader_pixel),
            (ShaderStage::Compute, &self.shader_compute),
        ]
    }

    pub fn all_valid_shaders(&self) -> Vec<(ShaderStage, &STechniqueStage)> {
        self.all_shaders()
            .into_iter()
            .filter(|(_, s)| s.shader.is_some())
            .collect()
    }
}

/// Indicates what to bind
///
///     `VertexPixel` - bind vs+ps, unbind gs+hs+ds+cs (also does stuff with `gear_dye` scopes, hasn't been reversed yet)
///     `VertexOnly` - bind vs, unbind ps+gs+hs+ds+cs
///     `VertexGeometryPixel` - bind vs+gs+ps, unbind hs+ds+cs
///     `VertexPixelTesselated` - bind vs+hs+ds+ps, unbind gs+cs
///     `VertexOnlyTesselated` - bind vs+hs+ds, unbind ps+cs+gs
///     Compute - bind cs, unbind vs+gs+hs+ds+ps
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum TechniqueBindMode {
    VertexPixel = 1,
    VertexOnly = 2,
    VertexGeometryPixel = 3,
    VertexPixelTesselated = 4,
    VertexOnlyTesselated = 5,
    Compute = 6,
}

impl TigerReadable for TechniqueBindMode {
    fn read_ds_endian(
        reader: &mut dyn TigerReader,
        endian: tiger_parse::Endian,
    ) -> tiger_parse::Result<Self> {
        let v = u32::read_ds_endian(reader, endian)?;
        Self::try_from(v).map_err(|_e| tiger_parse::Error::EnumVariantOutOfRange(v as usize))
    }

    const SIZE: usize = 4;
}

#[derive(Clone)]
#[tiger_type(size = 0x88)]
pub struct STechniqueStage {
    pub shader: TagHash,
    pub unk4: u32,
    pub core: SDynamicCore,
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x808086C6)]
pub struct SMaterialTextureAssignment {
    /// Material slot to assign to
    pub slot: u32,
    _pad: Padding<4>,
    pub texture: WideHash,
}

#[derive(Clone)]
#[tiger_type(size = 0x80)]
pub struct SDynamicCore {
    pub textures: Vec<SMaterialTextureAssignment>,
    pub unk10: u64,
    pub bytecode: Vec<u8>,                // 0x18
    pub bytecode_constants: Vec<Vec4>,    // 0x28
    pub samplers: Vec<SSamplerReference>, // 0x38
    pub unk30: Vec<Vec4>,                 // 0x48
    pub unk40: [u32; 2],                  // 0x58

    pub constant_buffer_slot: i32, // 0x60
    pub constant_buffer: TagHash,  // 0x64

    pub unk68: [u32; 6], // 0x68
}

#[derive(Clone)]
#[tiger_type(id = 0x8080013F)]
pub struct SSamplerReference {
    pub sampler: TagHash,
    pub unk4: u32,
    pub unk8: u32,
    pub unkc: u32,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct TfxScopeBits: u64 {
        const FRAME                            = 1 << 0;
        const VIEW                             = 1 << 1;
        const RIGID_MODEL                      = 1 << 2;
        const EDITOR_MESH                      = 1 << 3;
        const EDITOR_TERRAIN                   = 1 << 4;
        const CUI_VIEW                         = 1 << 5;
        const CUI_OBJECT                       = 1 << 6;
        const SKINNING                         = 1 << 7;
        const SPEEDTREE                        = 1 << 8;
        const CHUNK_MODEL                      = 1 << 9;
        const DECAL                            = 1 << 10;
        const INSTANCES                        = 1 << 11;
        const SPEEDTREE_LOD_DRAWCALL_DATA      = 1 << 12;
        const TRANSPARENT                      = 1 << 13;
        const TRANSPARENT_ADVANCED             = 1 << 14;
        const SDSM_BIAS_AND_SCALE_TEXTURES     = 1 << 15;
        const TERRAIN                          = 1 << 16;
        const POSTPROCESS                      = 1 << 17;
        const CUI_BITMAP                       = 1 << 18;
        const CUI_STANDARD                     = 1 << 19;
        const UI_FONT                          = 1 << 20;
        const CUI_HUD                          = 1 << 21;
        const PARTICLE_TRANSFORMS              = 1 << 22;
        const PARTICLE_LOCATION_METADATA       = 1 << 23;
        const CUBEMAP_VOLUME                   = 1 << 24;
        const GEAR_PLATED_TEXTURES             = 1 << 25;
        const GEAR_DYE_0                       = 1 << 26;
        const GEAR_DYE_1                       = 1 << 27;
        const GEAR_DYE_2                       = 1 << 28;
        const GEAR_DYE_DECAL                   = 1 << 29;
        const GENERIC_ARRAY                    = 1 << 30;
        const GEAR_DYE_SKIN                    = 1 << 31;
        const GEAR_DYE_LIPS                    = 1 << 32;
        const GEAR_DYE_HAIR                    = 1 << 33;
        const GEAR_DYE_FACIAL_LAYER_0_MASK     = 1 << 34;
        const GEAR_DYE_FACIAL_LAYER_0_MATERIAL = 1 << 35;
        const GEAR_DYE_FACIAL_LAYER_1_MASK     = 1 << 36;
        const GEAR_DYE_FACIAL_LAYER_1_MATERIAL = 1 << 37;
        const PLAYER_CENTERED_CASCADED_GRID    = 1 << 38;
        const GEAR_DYE_012                     = 1 << 39;
        const COLOR_GRADING_UBERSHADER         = 1 << 40;
        const CUI_DRAWING                      = 1 << 41;
    }
}

// TODO(cohae): tiger-parse doesnt work with bitflags, so we have to implement this manually
impl TigerReadable for TfxScopeBits {
    fn read_ds_endian(reader: &mut dyn TigerReader, endian: Endian) -> tiger_parse::Result<Self> {
        let bits: u64 = u64::read_ds_endian(reader, endian)?;
        Ok(Self::from_bits_truncate(bits))
    }

    const SIZE: usize = 8;
}

/// Current indices for blend, rasterizer, depth bias and depth stencil states
#[tiger_type(size = 4)]
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
pub struct FixedFunctionState {
    blend_state: u8,
    depth_stencil_state: u8,
    rasterizer_state: u8,
    depth_bias_state: u8,
}

impl FixedFunctionState {
    pub fn new(
        blend_state: Option<usize>,
        depth_stencil_state: Option<usize>,
        rasterizer_state: Option<usize>,
        depth_bias_state: Option<usize>,
    ) -> Self {
        Self {
            blend_state: blend_state.map_or(0, |v| v | 0x80) as u8,
            depth_stencil_state: depth_stencil_state.map_or(0, |v| v | 0x80) as u8,
            rasterizer_state: rasterizer_state.map_or(0, |v| v | 0x80) as u8,
            depth_bias_state: depth_bias_state.map_or(0, |v| v | 0x80) as u8,
        }
    }

    #[inline(always)]
    pub const fn from_raw(raw: u32) -> Self {
        Self {
            blend_state: (raw & 0xff) as u8,
            depth_stencil_state: ((raw >> 8) & 0xff) as u8,
            rasterizer_state: ((raw >> 16) & 0xff) as u8,
            depth_bias_state: ((raw >> 24) & 0xff) as u8,
        }
    }

    #[inline(always)]
    pub const fn raw(&self) -> u32 {
        (self.blend_state as u32)
            | ((self.depth_stencil_state as u32) << 8)
            | ((self.rasterizer_state as u32) << 16)
            | ((self.depth_bias_state as u32) << 24)
    }

    /// Creates a new selection, filling unset states in `other` with the default state in `self`
    pub const fn select(&self, other: &Self) -> Self {
        let current = self.raw();
        let other = other.raw();
        let new_states = ((other >> 7 & 0x1010101) * 0xff) & (current ^ other) ^ current;

        Self::from_raw(new_states)

        // PipelineState::new(
        //     other.blend_state().or_else(|| self.blend_state()),
        //     other
        //         .depth_stencil_state()
        //         .or_else(|| self.depth_stencil_state()),
        //     other.rasterizer_state().or_else(|| self.rasterizer_state()),
        //     other.depth_bias_state().or_else(|| self.depth_bias_state()),
        // )
    }

    pub const fn blend_state(&self) -> Option<usize> {
        if self.blend_state & 0x80 != 0 {
            Some((self.blend_state & 0x7f) as usize)
        } else {
            None
        }
    }

    pub const fn depth_stencil_state(&self) -> Option<usize> {
        if self.depth_stencil_state & 0x80 != 0 {
            Some((self.depth_stencil_state & 0x7f) as usize)
        } else {
            None
        }
    }

    pub const fn rasterizer_state(&self) -> Option<usize> {
        if self.rasterizer_state & 0x80 != 0 {
            Some((self.rasterizer_state & 0x7f) as usize)
        } else {
            None
        }
    }

    pub const fn depth_bias_state(&self) -> Option<usize> {
        if self.depth_bias_state & 0x80 != 0 {
            Some((self.depth_bias_state & 0x7f) as usize)
        } else {
            None
        }
    }

    pub const fn reset(&mut self) {
        self.blend_state = 0;
        self.depth_stencil_state = 0;
        self.rasterizer_state = 0;
        self.depth_bias_state = 0;
    }
}

impl Debug for FixedFunctionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateSelection")
            .field("blend_state", &self.blend_state())
            .field("depth_stencil_state", &self.depth_stencil_state())
            .field("rasterizer_state", &self.rasterizer_state())
            .field("depth_bias_state", &self.depth_bias_state())
            .finish()
    }
}
