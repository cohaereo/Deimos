use tiger_parse::{tiger_type, FnvHash, NullString, Pointer, ResourcePointer};
use tiger_pkg::TagHash;

use crate::tag::{TagRef, WideHash};

#[derive(Debug)]
#[tiger_type(id = 0x8080B383)]
pub struct SDestination {
    pub file_size: u64,
    pub location_name: FnvHash,
    pub unkc: u32,

    pub string_container: WideHash,

    pub events: TagHash,
    pub patrols: TagHash,
    pub unk28: u32,
    pub unk2c: u32,

    pub tag_bags: Vec<TagHash>, // 0x30

    pub unk40: u32,
    pub unk48: u32,
    pub activities: Vec<()>,
    pub destination_name: Pointer<NullString>,
}

#[derive(Debug)]
#[tiger_type(id = 0x8080B386, size = 0x104)]
pub struct SActivity {
    pub file_size: u64,
    pub location_name: FnvHash,
    pub unkc: FnvHash,
    pub unk10: FnvHash,
    pub unk14: FnvHash,
    pub unk18: ResourcePointer,
    pub destination: WideHash,

    #[tiger(offset = 0x40)]
    pub unk40: Vec<SUnk80808926>,
    pub unk50: Vec<SUnk80808924>,
    pub unk60: TagHash,
    pub unk64: TagHash,
    pub ambient_activity: WideHash,
}

#[derive(Debug)]
#[tiger_type(id = 0x8080AB3E, size = 0x38)]
pub struct SUnk80808924 {
    pub location_name: FnvHash,
    pub activity_name: FnvHash,
    pub bubble_name: FnvHash,
    pub unkc: u32,

    pub unk10: ResourcePointer,
    pub unk18: Vec<SUnk80808948>,
    pub map_references: Vec<WideHash>,
}

#[derive(Debug)]
#[tiger_type(id = 0x8FFFFFFF)]
pub struct SUnk80808926 {
    pub location_name: FnvHash,
    pub activity_name: FnvHash,
    pub bubble_name: FnvHash,
    pub unkc: FnvHash,
    pub unk10: FnvHash,
    pub unk14: u32,
    pub bubble_name2: FnvHash,
    pub unk1c: u32,
    pub unk20: FnvHash,
    pub unk24: FnvHash,
    pub unk28: FnvHash,
    pub unk2c: FnvHash,
    pub unk30: FnvHash,
    pub unk34: FnvHash,
    pub unk38: FnvHash,
    pub unk3c: u32,
    pub unk40: u32,
    pub unk44: u32,

    pub unk48: u32,
    pub unk4c: u32,
    pub unk50: Vec<SUnk80808948>,
    pub unk60: [u32; 4],
}

#[derive(Debug)]
#[tiger_type(id = 0x8080B463)]
pub struct SUnk80808948 {
    pub location_name: FnvHash,
    pub activity_name: FnvHash,
    pub bubble_name: FnvHash,
    pub activity_phase_name: FnvHash,
    pub activity_phase_name2: FnvHash,
    pub unk_entity_reference: TagRef<SUnk80808e89>,
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080B381)]
pub struct SUnk80808e89 {
    pub file_size: u64,
    pub unk8: u64,
    pub unk10: ResourcePointer,
    pub unk18: TagRef<SUnk80808ebe>,
    pub unk1c: u32,
    pub unk20: [u32; 4],
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080B3EA)]
pub struct SUnk80808ebe {
    pub file_size: u64,
    pub components: Vec<TagRef<Unk80808943>>,
}

#[derive(Debug, Clone)]
#[tiger_type(id = 0x8080AB82, size = 0x24)]
pub struct Unk80808943 {
    pub file_size: u64,
    #[tiger(offset = 0x20)]
    pub component: TagHash,
}
