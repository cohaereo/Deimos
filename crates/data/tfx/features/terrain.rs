use std::ops::Range;

use glam::Vec4;
use int_enum::IntEnum;
use tiger_parse::{tiger_type, TigerReadable, TigerReader};
use tiger_pkg::TagHash;

use crate::tfx::geometry::AxisAlignedBBox;

/// Terrain
#[derive(Debug)]
#[tiger_type(id = 0x80808567, size = 0xA4)]
pub struct STerrain {
    pub file_size: u64,
    pub unk8: u64,

    pub bounds: AxisAlignedBBox,
    pub unk30: Vec4,

    #[tiger(offset = 0x50)]
    pub mesh_groups: Vec<STerrainMeshGroup>,

    pub vertex0_buffer: TagHash,
    pub vertex1_buffer: TagHash,
    pub index_buffer: TagHash,
    pub technique_shadow: TagHash,
    pub technique_depth_only: TagHash,

    #[tiger(offset = 0x78)]
    pub mesh_parts: Vec<STerrainMeshPart>,

    #[tiger(offset = 0x88)]
    pub thumb_vertex0_buffer: TagHash,
    pub thumb_vertex1_buffer: TagHash,
    pub thumb_index_buffer: TagHash,
}

#[derive(Debug)]
#[tiger_type(id = 0x8080856C)]
pub struct STerrainMeshGroup {
    pub unk0: Vec4,
    pub unk10: f32,
    pub unk14: f32,
    pub unk18: f32,
    pub unk1c: u32,
    pub unk20: Vec4,
    pub unk30: u32,
    pub unk34: u32,
    pub unk38: u32,
    pub unk3c: u32,
    pub unk40: u32,
    pub unk44: u32,
    pub unk48: u32,
    pub unk4c: u32,
    pub dyemap: TagHash,
    pub unk54: u32,
    pub unk58: u32,
    pub unk5c: u32,
}

#[derive(Debug)]
#[tiger_type(id = 0x8080856A)]
pub struct STerrainMeshPart {
    pub technique: TagHash,
    pub index_start: u32,
    pub index_count: u16,
    pub group_index: u8,
    pub detail_level: TerrainDetailLevel,
}

impl STerrainMeshPart {
    pub const fn index_range(&self) -> Range<u32> {
        self.index_start..self.index_start + self.index_count as u32
    }
}

#[repr(u8)]
#[derive(Debug, IntEnum, PartialEq, Eq, PartialOrd)]
pub enum TerrainDetailLevel {
    High = 0,
    Medium = 1,
    Low = 2,
    Thumbnail = 3,
}

impl TigerReadable for TerrainDetailLevel {
    fn read_ds_endian(
        reader: &mut dyn TigerReader,
        endian: tiger_parse::Endian,
    ) -> tiger_parse::Result<Self> {
        let v = u8::read_ds_endian(reader, endian)?;
        Self::try_from(v).map_err(|_e| tiger_parse::Error::EnumVariantOutOfRange(v as usize))
    }

    const SIZE: usize = 1;
}
