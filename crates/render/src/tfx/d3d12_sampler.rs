use std::hash::Hash;

use ash::vk;

#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct D3D12SamplerDesc {
    pub filter: D3D12Filter,
    pub address_u: D3D12TextureAddressMode,
    pub address_v: D3D12TextureAddressMode,
    pub address_w: D3D12TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: D3D12ComparisonFunc,
    pub border_color: [f32; 4],
    pub min_lod: f32,
    pub max_lod: f32,
}

impl D3D12SamplerDesc {
    pub fn to_vk(&self) -> vk::SamplerCreateInfo<'_> {
        let (mip_filter, min_filter, mag_filter, anisotropy_enable) = self.filter.to_vk();

        vk::SamplerCreateInfo::default()
            .flags(vk::SamplerCreateFlags::empty())
            .mag_filter(mag_filter)
            .min_filter(min_filter)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(self.address_u.to_vk())
            .address_mode_v(self.address_v.to_vk())
            .address_mode_w(self.address_w.to_vk())
            .mip_lod_bias(0.0)
            .anisotropy_enable(anisotropy_enable)
            .max_anisotropy(self.max_anisotropy as f32)
            .compare_enable(self.comparison_func != D3D12ComparisonFunc::None)
            .compare_op(self.comparison_func.to_vk())
            .min_lod(self.min_lod)
            .max_lod(self.max_lod)
            // TODO: translate border color, don't think marathon uses this right now
            .border_color(vk::BorderColor::FLOAT_TRANSPARENT_BLACK)
            .unnormalized_coordinates(false)
    }
}

impl Eq for D3D12SamplerDesc {}

impl Hash for D3D12SamplerDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let raw = unsafe {
            std::slice::from_raw_parts((self as *const Self).cast::<u8>(), size_of::<Self>())
        };
        state.write(raw);
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum D3D12ComparisonFunc {
    None = 0,
    Never = 1,
    Less = 2,
    Equal = 3,
    LessEqual = 4,
    Greater = 5,
    NotEqual = 6,
    GreaterEqual = 7,
    Always = 8,
}

impl D3D12ComparisonFunc {
    pub fn to_vk(self) -> vk::CompareOp {
        match self {
            D3D12ComparisonFunc::None => vk::CompareOp::NEVER,
            D3D12ComparisonFunc::Never => vk::CompareOp::NEVER,
            D3D12ComparisonFunc::Less => vk::CompareOp::LESS,
            D3D12ComparisonFunc::Equal => vk::CompareOp::EQUAL,
            D3D12ComparisonFunc::LessEqual => vk::CompareOp::LESS_OR_EQUAL,
            D3D12ComparisonFunc::Greater => vk::CompareOp::GREATER,
            D3D12ComparisonFunc::NotEqual => vk::CompareOp::NOT_EQUAL,
            D3D12ComparisonFunc::GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
            D3D12ComparisonFunc::Always => vk::CompareOp::ALWAYS,
        }
    }
}

#[repr(u32)]
#[rustfmt::skip]
#[derive(Debug, Clone, Copy, Hash, PartialEq)]
pub enum D3D12Filter {
    MinMagMipPoint = 0x0,
    MinMagPointMipLinear = 0x1,
    MinPointMagLinearMipPoint = 0x4,
    MinPointMagMipLinear = 0x5,
    MinLinearMagMipPoint = 0x10,
    MinLinearMagPointMipLinear = 0x11,
    MinMagLinearMipPoint = 0x14,
    MinMagMipLinear = 0x15,
    Anisotropic = 0x55,
    ComparisonMinMagMipPoint = 0x80,
    ComparisonMinMagPointMipLinear = 0x81,
    ComparisonMinPointMagLinearMipPoint = 0x84,
    ComparisonMinPointMagMipLinear = 0x85,
    ComparisonMinLinearMagMipPoint = 0x90,
    ComparisonMinLinearMagPointMipLinear = 0x91,
    ComparisonMinMagLinearMipPoint = 0x94,
    ComparisonMinMagMipLinear = 0x95,
    ComparisonAnisotropic = 0xd5,
    MinimumMinMagMipPoint = 0x100,
    MinimumMinMagPointMipLinear = 0x101,
    MinimumMinPointMagLinearMipPoint = 0x104,
    MinimumMinPointMagMipLinear = 0x105,
    MinimumMinLinearMagMipPoint = 0x110,
    MinimumMinLinearMagPointMipLinear = 0x111,
    MinimumMinMagLinearMipPoint = 0x114,
    MinimumMinMagMipLinear = 0x115,
    MinimumAnisotropic = 0x155,
    MaximumMinMagMipPoint = 0x180,
    MaximumMinMagPointMipLinear = 0x181,
    MaximumMinPointMagLinearMipPoint = 0x184,
    MaximumMinPointMagMipLinear = 0x185,
    MaximumMinLinearMagMipPoint = 0x190,
    MaximumMinLinearMagPointMipLinear = 0x191,
    MaximumMinMagLinearMipPoint = 0x194,
    MaximumMinMagMipLinear = 0x195,
    MaximumAnisotropic = 0x1d5
}

impl D3D12Filter {
    pub const MIP_FILTER_SHIFT: u32 = 0;
    pub const MAG_FILTER_SHIFT: u32 = 2;
    pub const MIN_FILTER_SHIFT: u32 = 4;
    pub const ANISOTROPIC_FILTERING_BIT: u32 = 0x40;

    pub fn to_vk(self) -> (vk::Filter, vk::Filter, vk::Filter, bool) {
        let bits = self as u32;
        let mip = (bits >> Self::MIP_FILTER_SHIFT) & 0x3;
        let mag = (bits >> Self::MAG_FILTER_SHIFT) & 0x3;
        let min = (bits >> Self::MIN_FILTER_SHIFT) & 0x3;
        let anisotropic = (bits & Self::ANISOTROPIC_FILTERING_BIT) != 0;

        // Filters happen to be 0 for nearest/point and 1 for linear filtering in both D3D12 and Vulkan
        (
            vk::Filter::from_raw(mip as i32),
            vk::Filter::from_raw(mag as i32),
            vk::Filter::from_raw(min as i32),
            anisotropic,
        )
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum D3D12TextureAddressMode {
    Invalid = 0,
    Wrap = 1,
    Mirror = 2,
    Clamp = 3,
    Border = 4,
    MirrorOnce = 5,
}

impl D3D12TextureAddressMode {
    pub fn to_vk(self) -> vk::SamplerAddressMode {
        match self {
            D3D12TextureAddressMode::Invalid => vk::SamplerAddressMode::REPEAT,
            D3D12TextureAddressMode::Wrap => vk::SamplerAddressMode::REPEAT,
            D3D12TextureAddressMode::Mirror => vk::SamplerAddressMode::MIRRORED_REPEAT,
            D3D12TextureAddressMode::Clamp => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            D3D12TextureAddressMode::Border => vk::SamplerAddressMode::CLAMP_TO_BORDER,
            D3D12TextureAddressMode::MirrorOnce => vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE,
        }
    }
}
