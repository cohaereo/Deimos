use int_enum::IntEnum;
use tiger_parse::tiger_type;
use tiger_parse::TigerReadable;
use tiger_parse::TigerReader;
use tiger_pkg::TagHash;

#[derive(Debug)]
#[tiger_type(etype = 32, size = 0x40)]
pub struct STextureHeader {
    pub data_size: u32,
    pub format: DxgiFormat,
    pub _unk8: u32,

    #[tiger(offset = 0x20)]
    pub cafe: u16,

    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub array_size: u16,
    pub tile_count: u16,

    pub unk2c: u8,
    // #[deprecated(note = "Value is wrong. Use `mip_count` method to calculate mip count")]
    mip_count_broken: u8,
    pub unk2e: [u8; 10],
    pub unk38: u32,

    /// Optional
    pub large_buffer: TagHash,
}

impl STextureHeader {
    pub fn mip_count(&self) -> u16 {
        let largest_dim = self.width.max(self.height).max(self.depth).max(1);
        let calculated_mip_count = largest_dim.ilog2() as u16 + 1;

        calculated_mip_count.min(self.mip_count_broken as u16)
    }
}

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
pub enum DxgiFormat {
    UNKNOWN = 0,
    R32G32B32A32_TYPELESS = 1,
    R32G32B32A32_FLOAT = 2,
    R32G32B32A32_UINT = 3,
    R32G32B32A32_SINT = 4,
    R32G32B32_TYPELESS = 5,
    R32G32B32_FLOAT = 6,
    R32G32B32_UINT = 7,
    R32G32B32_SINT = 8,
    R16G16B16A16_TYPELESS = 9,
    R16G16B16A16_FLOAT = 10,
    R16G16B16A16_UNORM = 11,
    R16G16B16A16_UINT = 12,
    R16G16B16A16_SNORM = 13,
    R16G16B16A16_SINT = 14,
    R32G32_TYPELESS = 15,
    R32G32_FLOAT = 16,
    R32G32_UINT = 17,
    R32G32_SINT = 18,
    R32G8X24_TYPELESS = 19,
    D32_FLOAT_S8X24_UINT = 20,
    R32_FLOAT_X8X24_TYPELESS = 21,
    X32_TYPELESS_G8X24_UINT = 22,
    R10G10B10A2_TYPELESS = 23,
    R10G10B10A2_UNORM = 24,
    R10G10B10A2_UINT = 25,
    R11G11B10_FLOAT = 26,
    R8G8B8A8_TYPELESS = 27,
    R8G8B8A8_UNORM = 28,
    R8G8B8A8_UNORM_SRGB = 29,
    R8G8B8A8_UINT = 30,
    R8G8B8A8_SNORM = 31,
    R8G8B8A8_SINT = 32,
    R16G16_TYPELESS = 33,
    R16G16_FLOAT = 34,
    R16G16_UNORM = 35,
    R16G16_UINT = 36,
    R16G16_SNORM = 37,
    R16G16_SINT = 38,
    R32_TYPELESS = 39,
    D32_FLOAT = 40,
    R32_FLOAT = 41,
    R32_UINT = 42,
    R32_SINT = 43,
    R24G8_TYPELESS = 44,
    D24_UNORM_S8_UINT = 45,
    R24_UNORM_X8_TYPELESS = 46,
    X24_TYPELESS_G8_UINT = 47,
    R8G8_TYPELESS = 48,
    R8G8_UNORM = 49,
    R8G8_UINT = 50,
    R8G8_SNORM = 51,
    R8G8_SINT = 52,
    R16_TYPELESS = 53,
    R16_FLOAT = 54,
    D16_UNORM = 55,
    R16_UNORM = 56,
    R16_UINT = 57,
    R16_SNORM = 58,
    R16_SINT = 59,
    R8_TYPELESS = 60,
    R8_UNORM = 61,
    R8_UINT = 62,
    R8_SNORM = 63,
    R8_SINT = 64,
    A8_UNORM = 65,
    R1_UNORM = 66,
    R9G9B9E5_SHAREDEXP = 67,
    R8G8_B8G8_UNORM = 68,
    G8R8_G8B8_UNORM = 69,
    BC1_TYPELESS = 70,
    BC1_UNORM = 71,
    BC1_UNORM_SRGB = 72,
    BC2_TYPELESS = 73,
    BC2_UNORM = 74,
    BC2_UNORM_SRGB = 75,
    BC3_TYPELESS = 76,
    BC3_UNORM = 77,
    BC3_UNORM_SRGB = 78,
    BC4_TYPELESS = 79,
    BC4_UNORM = 80,
    BC4_SNORM = 81,
    BC5_TYPELESS = 82,
    BC5_UNORM = 83,
    BC5_SNORM = 84,
    B5G6R5_UNORM = 85,
    B5G5R5A1_UNORM = 86,
    B8G8R8A8_UNORM = 87,
    B8G8R8X8_UNORM = 88,
    R10G10B10_XR_BIAS_A2_UNORM = 89,
    B8G8R8A8_TYPELESS = 90,
    B8G8R8A8_UNORM_SRGB = 91,
    B8G8R8X8_TYPELESS = 92,
    B8G8R8X8_UNORM_SRGB = 93,
    BC6H_TYPELESS = 94,
    BC6H_UF16 = 95,
    BC6H_SF16 = 96,
    BC7_TYPELESS = 97,
    BC7_UNORM = 98,
    BC7_UNORM_SRGB = 99,
    AYUV = 100,
    Y410 = 101,
    Y416 = 102,
    NV12 = 103,
    P010 = 104,
    P016 = 105,
    OPAQUE420 = 106,
    YUY2 = 107,
    Y210 = 108,
    Y216 = 109,
    NV11 = 110,
    AI44 = 111,
    IA44 = 112,
    P8 = 113,
    A8P8 = 114,
    B4G4R4A4_UNORM = 115,
    P208 = 130,
    V208 = 131,
    V408 = 132,
    SAMPLER_FEEDBACK_MIN_MIP_OPAQUE = 189,
    SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE = 190,
    A4B4G4R4_UNORM = 191,
}

impl TigerReadable for DxgiFormat {
    fn read_ds_endian(
        reader: &mut dyn TigerReader,
        endian: tiger_parse::Endian,
    ) -> tiger_parse::Result<Self> {
        let v = u32::read_ds_endian(reader, endian)?;
        Self::try_from(v).map_err(|_e| tiger_parse::Error::EnumVariantOutOfRange(v as usize))
    }

    const SIZE: usize = 4;
}

#[allow(unused)]
impl DxgiFormat {
    #[allow(clippy::match_same_arms)]
    pub fn bpp(&self) -> u32 {
        match self {
            Self::R32G32B32A32_TYPELESS
            | Self::R32G32B32A32_FLOAT
            | Self::R32G32B32A32_UINT
            | Self::R32G32B32A32_SINT => 128,
            Self::R32G32B32_TYPELESS
            | Self::R32G32B32_FLOAT
            | Self::R32G32B32_UINT
            | Self::R32G32B32_SINT => 96,
            Self::R16G16B16A16_TYPELESS
            | Self::R16G16B16A16_FLOAT
            | Self::R16G16B16A16_UNORM
            | Self::R16G16B16A16_UINT
            | Self::R16G16B16A16_SNORM
            | Self::R16G16B16A16_SINT
            | Self::R32G32_TYPELESS
            | Self::R32G32_FLOAT
            | Self::R32G32_UINT
            | Self::R32G32_SINT
            | Self::R32G8X24_TYPELESS
            | Self::D32_FLOAT_S8X24_UINT
            | Self::R32_FLOAT_X8X24_TYPELESS
            | Self::X32_TYPELESS_G8X24_UINT
            | Self::Y416
            | Self::Y210
            | Self::Y216 => 64,
            Self::R10G10B10A2_TYPELESS
            | Self::R10G10B10A2_UNORM
            | Self::R10G10B10A2_UINT
            | Self::R11G11B10_FLOAT
            | Self::R8G8B8A8_TYPELESS
            | Self::R8G8B8A8_UNORM
            | Self::R8G8B8A8_UNORM_SRGB
            | Self::R8G8B8A8_UINT
            | Self::R8G8B8A8_SNORM
            | Self::R8G8B8A8_SINT
            | Self::R16G16_TYPELESS
            | Self::R16G16_FLOAT
            | Self::R16G16_UNORM
            | Self::R16G16_UINT
            | Self::R16G16_SNORM
            | Self::R16G16_SINT
            | Self::R32_TYPELESS
            | Self::D32_FLOAT
            | Self::R32_FLOAT
            | Self::R32_UINT
            | Self::R32_SINT
            | Self::R24G8_TYPELESS
            | Self::D24_UNORM_S8_UINT
            | Self::R24_UNORM_X8_TYPELESS
            | Self::X24_TYPELESS_G8_UINT
            | Self::R9G9B9E5_SHAREDEXP
            | Self::R8G8_B8G8_UNORM
            | Self::G8R8_G8B8_UNORM
            | Self::B8G8R8A8_UNORM
            | Self::B8G8R8X8_UNORM
            | Self::R10G10B10_XR_BIAS_A2_UNORM
            | Self::B8G8R8A8_TYPELESS
            | Self::B8G8R8A8_UNORM_SRGB
            | Self::B8G8R8X8_TYPELESS
            | Self::B8G8R8X8_UNORM_SRGB
            | Self::AYUV
            | Self::Y410
            | Self::YUY2 => 32,
            Self::P010 | Self::P016 => 24,
            Self::R8G8_TYPELESS
            | Self::R8G8_UNORM
            | Self::R8G8_UINT
            | Self::R8G8_SNORM
            | Self::R8G8_SINT
            | Self::R16_TYPELESS
            | Self::R16_FLOAT
            | Self::D16_UNORM
            | Self::R16_UNORM
            | Self::R16_UINT
            | Self::R16_SNORM
            | Self::R16_SINT
            | Self::B5G6R5_UNORM
            | Self::B5G5R5A1_UNORM
            | Self::A8P8
            | Self::B4G4R4A4_UNORM => 16,
            Self::NV12 | Self::OPAQUE420 | Self::NV11 => 12,
            Self::R8_TYPELESS
            | Self::R8_UNORM
            | Self::R8_UINT
            | Self::R8_SNORM
            | Self::R8_SINT
            | Self::A8_UNORM
            | Self::AI44
            | Self::IA44
            | Self::P8 => 8,
            Self::R1_UNORM => 1,
            Self::BC1_TYPELESS
            | Self::BC1_UNORM
            | Self::BC1_UNORM_SRGB
            | Self::BC4_TYPELESS
            | Self::BC4_UNORM
            | Self::BC4_SNORM => 4,
            Self::BC2_TYPELESS
            | Self::BC2_UNORM
            | Self::BC2_UNORM_SRGB
            | Self::BC3_TYPELESS
            | Self::BC3_UNORM
            | Self::BC3_UNORM_SRGB
            | Self::BC5_TYPELESS
            | Self::BC5_UNORM
            | Self::BC5_SNORM
            | Self::BC6H_TYPELESS
            | Self::BC6H_UF16
            | Self::BC6H_SF16
            | Self::BC7_TYPELESS
            | Self::BC7_UNORM
            | Self::BC7_UNORM_SRGB => 8,
            u => panic!("{u:?}"),
        }
    }

    pub const fn is_srgb(&self) -> bool {
        matches!(
            self,
            Self::R8G8B8A8_UNORM_SRGB
                | Self::BC1_UNORM_SRGB
                | Self::BC2_UNORM_SRGB
                | Self::BC3_UNORM_SRGB
                | Self::B8G8R8A8_UNORM_SRGB
                | Self::B8G8R8X8_UNORM_SRGB
                | Self::BC7_UNORM_SRGB
        )
    }

    pub const fn is_compressed(&self) -> bool {
        matches!(
            self,
            Self::BC1_TYPELESS
                | Self::BC1_UNORM
                | Self::BC1_UNORM_SRGB
                | Self::BC4_TYPELESS
                | Self::BC4_UNORM
                | Self::BC4_SNORM
                | Self::BC2_TYPELESS
                | Self::BC2_UNORM
                | Self::BC2_UNORM_SRGB
                | Self::BC3_TYPELESS
                | Self::BC3_UNORM
                | Self::BC3_UNORM_SRGB
                | Self::BC5_TYPELESS
                | Self::BC5_UNORM
                | Self::BC5_SNORM
                | Self::BC6H_TYPELESS
                | Self::BC6H_UF16
                | Self::BC6H_SF16
                | Self::BC7_TYPELESS
                | Self::BC7_UNORM
                | Self::BC7_UNORM_SRGB
        )
    }

    pub fn calculate_pitch(&self, width: u32, height: u32) -> (usize, usize) {
        match self {
            Self::BC1_TYPELESS
            | Self::BC1_UNORM
            | Self::BC1_UNORM_SRGB
            | Self::BC4_TYPELESS
            | Self::BC4_UNORM
            | Self::BC4_SNORM => {
                let nbw = ((width as i64 + 3) / 4).clamp(1, i64::MAX) as usize;
                let nbh = ((height as i64 + 3) / 4).clamp(1, i64::MAX) as usize;

                let pitch = nbw * 8;
                (pitch, pitch * nbh)
            }
            Self::BC2_TYPELESS
            | Self::BC2_UNORM
            | Self::BC2_UNORM_SRGB
            | Self::BC3_TYPELESS
            | Self::BC3_UNORM
            | Self::BC3_UNORM_SRGB
            | Self::BC5_TYPELESS
            | Self::BC5_UNORM
            | Self::BC5_SNORM
            | Self::BC6H_TYPELESS
            | Self::BC6H_UF16
            | Self::BC6H_SF16
            | Self::BC7_TYPELESS
            | Self::BC7_UNORM
            | Self::BC7_UNORM_SRGB => {
                let nbw = ((width as i64 + 3) / 4).clamp(1, i64::MAX) as usize;
                let nbh = ((height as i64 + 3) / 4).clamp(1, i64::MAX) as usize;

                let pitch = nbw * 16;
                (pitch, pitch * nbh)
            }
            _ => {
                let pitch = (width * self.bpp()).div_ceil(8) as usize;
                (pitch, height as usize * pitch)
            }
        }
    }
}
