use std::io::SeekFrom;

use assert_offset::AssertOffsets;
use glam::{Quat, Vec4};
use tiger_parse::{
    tiger_type, tiger_variant_enum, Endian, FnvHash, Padding, TigerReadable, TigerReader,
    VariantEnum,
};
use tiger_pkg::TagHash;

use crate::{
    tag::{OptionalTagRef, TagRef, WideHash, WideTag},
    tfx::{
        features::{
            cubemap::SCubemapComponent,
            decals::SDecalCollection,
            decorators::SDecorator,
            light::{SLightCollection, SShadowingLight},
            sky_objects::SSkyObjectCollection,
        },
        geometry::AxisAlignedBBox,
    },
    umbra::SUmbraTomes,
};

#[derive(Debug, AssertOffsets)]
#[tiger_type(id = 0x8080AB27, size = 0x50)]
pub struct SBubbleParentShallow {
    pub file_size: u64,

    #[offset(0x8)]
    pub definition: TagHash,
    pub unkc: Padding<4>,

    pub unk10: u64,
    pub map_name: FnvHash,
}

#[derive(Debug, AssertOffsets)]
#[tiger_type(id = 0x8080AB27, size = 0x50)]
pub struct SBubbleParent {
    pub file_size: u64,

    #[offset(0x8)]
    pub definition: TagRef<SBubbleDefinition>,
    pub unkc: Padding<4>,

    pub unk10: u64,
    pub map_name: FnvHash,
}

#[derive(Debug, AssertOffsets)]
#[tiger_type(id = 0x8080A7BB, size = 0x50)]
pub struct SBubbleDefinition {
    pub file_size: u64,
    pub containers: Vec<WideTag<SMapContainer>>,
}

#[derive(Debug)]
#[tiger_type(id = 0x8080A7C1, size = 0x38)]
pub struct SMapContainer {
    pub file_size: u64,
    #[tiger(offset = 0x28)]
    pub data_tables: Vec<TagHash>,
}

#[tiger_type(id = 0x8080B1A7)]
pub struct SMapNodeTable {
    pub file_size: u64,
    pub nodes: Vec<SMapNodeEntry>,
}

#[tiger_type(id = 0x8080B3F5)]
pub struct SMapNodeEntry {
    pub rotation: Quat,
    pub translation: Vec4,
    pub entity_old: TagHash,
    pub unk24: u32,
    pub entity: WideHash,
    pub unk38: [u32; 9], //
    pub unk5c: f32,
    pub unk60: f32,
    pub unk64: TagHash,
    pub unk68: FnvHash,
    pub unk6c: u32,
    pub world_id: u64,
    pub component_data: SComponentDataListPtr,
    pub unk80: [u32; 4],
}

tiger_variant_enum! {
    [offset = 0x10]
    [Unknown(true)]
    enum ComponentData {
        SStaticTerrainPatchesComponent,
        SStaticInstancesCollectionComponent,
        SSkyObjectCollectionComponent,
        SDecalCollectionComponent,
        SDecoratorsComponent,
        SMaterialPermutationsComponent,
        SShadowingLightComponent,
        SLightCollectionComponent,
        SCubemapComponent,
        SUmbraTomeComponent,
        SWaterPlaneComponent
    }
}

#[derive(Debug)]
#[tiger_type(id = 0x8080402E)]
pub struct SMaterialPermutationsComponent {
    pub config: Vec<(u32, u32)>,
}

// #[tiger_type(id = 0x80806F38)]
// pub struct SStaticAmbientOcclusionComponent {
//     pub ao: Tag<SStaticAmbientOcclusion>,
// }

#[tiger_type(id = 0x808085B0)]
pub struct SStaticInstancesCollectionComponent {
    pub instances: TagHash,
}

#[tiger_type(id = 0x80808378)]
pub struct SSkyObjectCollectionComponent {
    pub objects: OptionalTagRef<SSkyObjectCollection>,
}

// #[tiger_type(id = 0x80806DE0)]
// pub struct SWaterPlaneComponent {
//     pub model: TagHash,
// }

#[tiger_type(id = 0x80808335)]
pub struct SLightCollectionComponent {
    pub lights: OptionalTagRef<SLightCollection>,
}

#[tiger_type(id = 0x80808544)]
pub struct SShadowingLightComponent {
    pub light: OptionalTagRef<SShadowingLight>,
}

#[tiger_type(id = 0x8080821E)]
pub struct SDecalCollectionComponent {
    pub decals: OptionalTagRef<SDecalCollection>,
}

#[derive(Clone, Debug)]
#[tiger_type(id = 0x80808563)]
pub struct SStaticTerrainPatchesComponent {
    pub identifier: u64,
    pub terrain: TagHash,
    pub terrain_bounds: TagHash,
}

#[tiger_type(id = 0x808085AA)]
pub struct SDecoratorsComponent {
    pub decorators: OptionalTagRef<SDecorator>,
}

#[tiger_type(id = 0x808085DE)]
pub struct SUmbraTomeComponent {
    pub tag: OptionalTagRef<SUmbraTomes>,
}

#[tiger_type(id = 0x8080819C)]
pub struct SWaterPlaneComponent {
    pub model: TagHash,
    pub unk4: u32,
    pub unk8: u32,
    pub unkc: u32,
    pub bounds: AxisAlignedBBox,
    pub collision_volume_hkx: TagHash,
    pub unk34: u32,
    pub unk38: TagHash,
}

#[derive(Debug)]
#[tiger_type(id = 0x808085E3)]
pub struct S808085E3 {
    pub unk0: u32,
    pub key: FnvHash,
    pub values: Vec<FnvHash>,
}

pub struct SComponentDataNode {
    next: Option<Box<Self>>,
    data: ComponentData,
}

impl SComponentDataNode {
    pub fn next(&self) -> Option<&Self> {
        self.next.as_deref()
    }

    pub const fn data(&self) -> &ComponentData {
        &self.data
    }
}

pub struct SComponentDataListPtr(Option<SComponentDataNode>);

impl SComponentDataListPtr {
    pub const fn iter<'a>(&'a self) -> ComponentDataListIter<'a> {
        ComponentDataListIter {
            current: self.0.as_ref(),
        }
    }

    pub const fn first(&self) -> Option<&SComponentDataNode> {
        self.0.as_ref()
    }

    pub fn get_by_class(&self, class_id: u32) -> Option<&ComponentData> {
        self.iter().find(|c| c.class_id() == class_id)
    }
}

impl TigerReadable for SComponentDataListPtr {
    fn read_ds_endian(reader: &mut dyn TigerReader, endian: Endian) -> tiger_parse::Result<Self> {
        let offset_base = reader.stream_position()?;
        let offset: i64 = TigerReadable::read_ds_endian(reader, endian)?;
        if offset == 0 || offset == i64::MAX {
            return Ok(Self(None));
        }

        let offset_save = reader.stream_position()?;

        reader.seek(SeekFrom::Start(offset_base))?;
        reader.seek(SeekFrom::Current(offset - 4))?;
        let resource_type: u32 = TigerReadable::read_ds_endian(reader, endian)?;

        reader.seek(SeekFrom::Start(offset_base))?;
        reader.seek(SeekFrom::Current(offset))?;
        let next_unboxed = Self::read_ds_endian(reader, endian)?;
        let next = next_unboxed.0.map(Box::new);

        reader.seek(SeekFrom::Start(offset_base))?;
        reader.seek(SeekFrom::Current(offset + 0x10))?;
        let data = ComponentData::read_variant_endian(reader, endian, resource_type)?;

        reader.seek(SeekFrom::Start(offset_save))?;

        Ok(Self(Some(SComponentDataNode { next, data })))
    }

    const ID: Option<u32> = None;

    const SIZE: usize = 8;
}

pub struct ComponentDataListIter<'a> {
    current: Option<&'a SComponentDataNode>,
}

impl<'a> Iterator for ComponentDataListIter<'a> {
    type Item = &'a ComponentData;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            &node.data
        })
    }
}
