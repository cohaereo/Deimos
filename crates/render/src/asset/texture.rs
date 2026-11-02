use std::{borrow::Cow, sync::Arc};

use anyhow::Context;
use ash::vk;
use deimos_data::{
    tag::WideHash,
    tfx::{
        ShaderStage,
        texture::{DxgiFormat, STextureHeader},
    },
};
use tiger_parse::PackageManagerExt;
use tiger_pkg::package_manager;
use tracing::debug_span;

use crate::gpu::{
    Gpu,
    alloc::{BufferAllocDesc, ImageAllocDesc, image::Image},
    command_buffer::{IMAGE_STATE_SHADER_READ, IMAGE_STATE_TRANSFER_DST},
    command_list::CommandList,
};

const COPY_ROW_PITCH_ALIGNMENT: usize = 256;

struct SubresourceFootprint {
    dst_offset: usize,
    src_offset: usize,
    src_row_pitch: usize,
    dst_row_pitch: usize,
    buffer_row_length: u32,
    mip: u32,
    array_layer: u32,
    width: u32,
    height: u32,
    depth: u32,
    block_height: usize,
}

fn align_up(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment > 0);
    value.div_ceil(alignment) * alignment
}

const fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

const fn lcm(a: usize, b: usize) -> usize {
    a / gcd(a, b) * b
}

fn texel_block_size(format: DxgiFormat) -> usize {
    if format.is_compressed() {
        format.calculate_pitch(4, 4).0
    } else {
        (format.bpp() as usize).div_ceil(8).max(1)
    }
}

fn build_footprints(
    desc: &TextureDesc<'_>,
    num_mips: usize,
    array_size: usize,
    is_3d: bool,
) -> (Vec<SubresourceFootprint>, usize, usize) {
    let compressed = desc.format.is_compressed();
    let block_size = texel_block_size(desc.format);
    let row_alignment = lcm(COPY_ROW_PITCH_ALIGNMENT, block_size);
    let offset_alignment = block_size.max(4);

    let mut footprints = Vec::with_capacity(num_mips * array_size);
    let mut dst_offset = 0usize;
    let mut src_offset = 0usize;

    for subresource in 0..num_mips * array_size {
        // Matches D3D12 subresource order: mip + array_layer * num_mips.
        let mip = subresource % num_mips;
        let array_layer = subresource / num_mips;

        let width = (desc.width >> mip).max(1);
        let height = (desc.height >> mip).max(1);
        let depth = if is_3d {
            ((desc.depth as u32) >> mip).max(1)
        } else {
            1
        };

        let (src_row_pitch, _) = desc.format.calculate_pitch(width, height);
        let block_height = if compressed {
            height.div_ceil(4)
        } else {
            height
        } as usize;

        let dst_row_pitch = align_up(src_row_pitch, row_alignment);
        let buffer_row_length = if compressed {
            ((dst_row_pitch / block_size) * 4) as u32
        } else {
            (dst_row_pitch / block_size) as u32
        };

        dst_offset = align_up(dst_offset, offset_alignment);
        let dst_size = dst_row_pitch * block_height * depth as usize;
        let src_size = src_row_pitch * block_height * depth as usize;

        footprints.push(SubresourceFootprint {
            dst_offset,
            src_offset,
            src_row_pitch,
            dst_row_pitch,
            buffer_row_length,
            mip: mip as u32,
            array_layer: array_layer as u32,
            width,
            height,
            depth,
            block_height,
        });

        dst_offset += dst_size;
        src_offset += src_size;
    }

    (footprints, dst_offset, src_offset)
}

pub struct Texture {
    pub view: vk::ImageView,
    pub image: Image,
    gpu: Arc<Gpu>,
}

impl Texture {
    pub fn load_data(
        hash: WideHash,
        load_full_mip: bool,
    ) -> anyhow::Result<(STextureHeader, Vec<u8>)> {
        let texture_header_ref = package_manager()
            .get_entry(hash)
            .with_context(|| format!("Texture header entry for {hash} not found"))?
            .reference;

        let texture: STextureHeader = package_manager().read_tag_struct(hash)?;
        let mut texture_data = if texture.large_buffer.is_some() {
            package_manager()
                .read_tag(texture.large_buffer)
                .context("Failed to read texture data")?
        } else {
            package_manager()
                .read_tag(texture_header_ref)
                .context("Failed to read texture data")?
        };

        if load_full_mip && texture.large_buffer.is_some() {
            let ab = package_manager()
                .read_tag(texture_header_ref)
                .context("Failed to read large texture buffer")?;

            texture_data.extend(ab);
        }

        Ok((texture, texture_data))
    }

    pub fn load_tag(gpu: &Arc<Gpu>, hash: tiger_pkg::TagHash) -> anyhow::Result<Self> {
        let hash = hash.into();
        let _span = debug_span!("Load texture", ?hash).entered();
        let (header, texture_data) = Self::load_data(hash, true)?;

        Self::load(
            gpu,
            &TextureDesc {
                image_type: if header.depth > 1 {
                    vk::ImageType::TYPE_3D
                } else {
                    vk::ImageType::TYPE_2D
                },
                name: format!("texture_tag {hash}").into(),
                format: header.format,
                width: header.width as u32,
                height: header.height as u32,
                depth: header.depth,
                array_size: header.array_size,
                num_mips: header.mip_count(),
            },
            &texture_data,
        )
    }

    pub fn load(gpu: &Arc<Gpu>, desc: &TextureDesc<'_>, data: &[u8]) -> anyhow::Result<Self> {
        let is_3d = desc.image_type == vk::ImageType::TYPE_3D;
        let num_mips = desc.num_mips.max(1) as usize;
        let array_size = if is_3d {
            1
        } else {
            desc.array_size.max(1) as usize
        };
        let (footprints, upload_size, source_size) =
            build_footprints(desc, num_mips, array_size, is_3d);

        anyhow::ensure!(
            data.len() >= source_size,
            "Texture data is too small: expected at least {source_size} bytes, got {}",
            data.len()
        );

        let staging = gpu.alloc_buffer(&BufferAllocDesc {
            name: "staging",
            size: upload_size as u64,
            usage: ash::vk::BufferUsageFlags::TRANSFER_SRC,
            flags: vk::BufferCreateFlags::empty(),
            location: gpu_allocator::MemoryLocation::CpuToGpu,
        })?;
        let ptr = staging.mapped_ptr();

        let dst = unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr(), upload_size) };
        for footprint in &footprints {
            let src_size =
                footprint.src_row_pitch * footprint.block_height * footprint.depth as usize;
            let src = data
                .get(footprint.src_offset..footprint.src_offset + src_size)
                .with_context(|| {
                    format!(
                        "Texture data out of range for mip {} (array layer {})",
                        footprint.mip, footprint.array_layer
                    )
                })?;

            for depth_slice in 0..footprint.depth as usize {
                for row in 0..footprint.block_height {
                    let src_start = depth_slice * footprint.src_row_pitch * footprint.block_height
                        + row * footprint.src_row_pitch;
                    let src_end = src_start + footprint.src_row_pitch;

                    let dst_start = footprint.dst_offset
                        + depth_slice * footprint.dst_row_pitch * footprint.block_height
                        + row * footprint.dst_row_pitch;
                    let dst_end = dst_start + footprint.src_row_pitch;

                    dst[dst_start..dst_end].copy_from_slice(&src[src_start..src_end]);
                }
            }
        }

        let mut image = gpu.alloc_image(&ImageAllocDesc {
            name: &desc.name,
            image_type: desc.image_type,
            format: dxgi_to_vk(desc.format).expect("Invalid texture format"),
            extent: vk::Extent3D {
                width: desc.width,
                height: desc.height,
                depth: if is_3d { desc.depth.max(1) as u32 } else { 1 },
            },
            mip_levels: num_mips as u32,
            array_layers: array_size as u32,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            location: gpu_allocator::MemoryLocation::GpuOnly,
        })?;

        let cmd = gpu.upload_command_buffer();
        image.layout_barrier(*cmd, vk::ImageAspectFlags::COLOR, IMAGE_STATE_TRANSFER_DST);

        let regions = footprints
            .iter()
            .map(|footprint| {
                vk::BufferImageCopy::default()
                    .buffer_offset(footprint.dst_offset as u64)
                    .buffer_row_length(footprint.buffer_row_length)
                    .buffer_image_height(0)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(footprint.mip)
                            .base_array_layer(footprint.array_layer)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: footprint.width,
                        height: footprint.height,
                        depth: footprint.depth,
                    })
            })
            .collect::<Vec<_>>();

        unsafe {
            gpu.cmd_copy_buffer_to_image(
                *cmd,
                staging.vk_buffer(),
                image.vk_image(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
        }

        image.layout_barrier(*cmd, vk::ImageAspectFlags::COLOR, IMAGE_STATE_SHADER_READ);

        // resource.resource().set_debug_name(&desc.name);
        let (view_type, layer_count) = match desc.image_type {
            vk::ImageType::TYPE_3D => (vk::ImageViewType::TYPE_3D, 1),
            vk::ImageType::TYPE_2D if array_size == 6 => {
                (vk::ImageViewType::CUBE, array_size as u32)
            }
            vk::ImageType::TYPE_2D if array_size > 1 => {
                (vk::ImageViewType::TYPE_2D_ARRAY, array_size as u32)
            }
            vk::ImageType::TYPE_2D => (vk::ImageViewType::TYPE_2D, 1),
            vk::ImageType::TYPE_1D if array_size > 1 => {
                (vk::ImageViewType::TYPE_1D_ARRAY, array_size as u32)
            }
            vk::ImageType::TYPE_1D => (vk::ImageViewType::TYPE_1D, 1),
            _ => unreachable!(),
        };
        let view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .format(dxgi_to_vk(desc.format).expect("Invalid texture format"))
                    .view_type(view_type)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(layer_count)
                            .level_count(num_mips as u32),
                    ),
                None,
            )?
        };

        Ok(Self {
            gpu: gpu.clone(),
            image,
            view,
        })
    }

    pub fn bind(&self, cmd: &mut CommandList, slot: u32, stage: ShaderStage) {
        cmd.set_shader_resource_view(stage, slot, Some(self.view));
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { self.gpu.destroy_image_view(self.view, None) };
    }
}

pub struct TextureDesc<'a> {
    pub image_type: vk::ImageType,
    pub name: Cow<'a, str>,
    pub format: DxgiFormat,
    pub width: u32,
    pub height: u32,
    pub depth: u16,
    pub array_size: u16,
    pub num_mips: u16,
}

impl<'a> TextureDesc<'a> {
    pub fn texture_2d(name: impl AsRef<str>, format: DxgiFormat, width: u32, height: u32) -> Self {
        Self {
            image_type: vk::ImageType::TYPE_2D,
            name: Cow::Owned(name.as_ref().to_string()),
            format,
            width,
            height,
            depth: 1,
            array_size: 1,
            num_mips: 1,
        }
    }

    pub fn texture_3d(
        name: impl AsRef<str>,
        format: DxgiFormat,
        width: u32,
        height: u32,
        depth: u32,
    ) -> Self {
        Self {
            image_type: vk::ImageType::TYPE_3D,
            name: Cow::Owned(name.as_ref().to_string()),
            format,
            width,
            height,
            depth: depth as u16,
            array_size: 1,
            num_mips: 1,
        }
    }
}

const fn dxgi_to_vk(format: DxgiFormat) -> Option<vk::Format> {
    // Adapted from https://github.com/HansKristian-Work/vkd3d-proton/blob/49a1e2c7098f4646374458b9a8fbc7c0d0b5ff3d/libs/vkd3d/utils.c#L48
    Some(match format {
        DxgiFormat::R32G32B32A32_TYPELESS => vk::Format::R32G32B32A32_SFLOAT,
        DxgiFormat::R32G32B32A32_FLOAT => vk::Format::R32G32B32A32_SFLOAT,
        DxgiFormat::R32G32B32A32_UINT => vk::Format::R32G32B32A32_UINT,
        DxgiFormat::R32G32B32A32_SINT => vk::Format::R32G32B32A32_SINT,
        DxgiFormat::R32G32B32_TYPELESS => vk::Format::R32G32B32_SFLOAT,
        DxgiFormat::R32G32B32_FLOAT => vk::Format::R32G32B32_SFLOAT,
        DxgiFormat::R32G32B32_UINT => vk::Format::R32G32B32_UINT,
        DxgiFormat::R32G32B32_SINT => vk::Format::R32G32B32_SINT,
        DxgiFormat::R16G16B16A16_TYPELESS => vk::Format::R16G16B16A16_SFLOAT,
        DxgiFormat::R16G16B16A16_FLOAT => vk::Format::R16G16B16A16_SFLOAT,
        DxgiFormat::R16G16B16A16_UNORM => vk::Format::R16G16B16A16_UNORM,
        DxgiFormat::R16G16B16A16_UINT => vk::Format::R16G16B16A16_UINT,
        DxgiFormat::R16G16B16A16_SNORM => vk::Format::R16G16B16A16_SNORM,
        DxgiFormat::R16G16B16A16_SINT => vk::Format::R16G16B16A16_SINT,
        DxgiFormat::R32G32_TYPELESS => vk::Format::R32G32_SFLOAT,
        DxgiFormat::R32G32_FLOAT => vk::Format::R32G32_SFLOAT,
        DxgiFormat::R32G32_UINT => vk::Format::R32G32_UINT,
        DxgiFormat::R32G32_SINT => vk::Format::R32G32_SINT,
        DxgiFormat::R10G10B10A2_TYPELESS => vk::Format::A2B10G10R10_UNORM_PACK32,
        DxgiFormat::R10G10B10A2_UNORM => vk::Format::A2B10G10R10_UNORM_PACK32,
        DxgiFormat::R10G10B10A2_UINT => vk::Format::A2B10G10R10_UINT_PACK32,
        DxgiFormat::R11G11B10_FLOAT => vk::Format::B10G11R11_UFLOAT_PACK32,
        DxgiFormat::R8G8_TYPELESS => vk::Format::R8G8_UNORM,
        DxgiFormat::R8G8_UNORM => vk::Format::R8G8_UNORM,
        DxgiFormat::R8G8_UINT => vk::Format::R8G8_UINT,
        DxgiFormat::R8G8_SNORM => vk::Format::R8G8_SNORM,
        DxgiFormat::R8G8_SINT => vk::Format::R8G8_SINT,
        DxgiFormat::R8G8B8A8_TYPELESS => vk::Format::R8G8B8A8_UNORM,
        DxgiFormat::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_UNORM,
        DxgiFormat::R8G8B8A8_UNORM_SRGB => vk::Format::R8G8B8A8_SRGB,
        DxgiFormat::R8G8B8A8_UINT => vk::Format::R8G8B8A8_UINT,
        DxgiFormat::R8G8B8A8_SNORM => vk::Format::R8G8B8A8_SNORM,
        DxgiFormat::R8G8B8A8_SINT => vk::Format::R8G8B8A8_SINT,
        DxgiFormat::R16G16_TYPELESS => vk::Format::R16G16_SFLOAT,
        DxgiFormat::R16G16_FLOAT => vk::Format::R16G16_SFLOAT,
        DxgiFormat::R16G16_UNORM => vk::Format::R16G16_UNORM,
        DxgiFormat::R16G16_UINT => vk::Format::R16G16_UINT,
        DxgiFormat::R16G16_SNORM => vk::Format::R16G16_SNORM,
        DxgiFormat::R16G16_SINT => vk::Format::R16G16_SINT,
        DxgiFormat::R32_TYPELESS => vk::Format::R32_UINT,
        DxgiFormat::D32_FLOAT => vk::Format::D32_SFLOAT,
        DxgiFormat::R32_FLOAT => vk::Format::R32_SFLOAT,
        DxgiFormat::R32_UINT => vk::Format::R32_UINT,
        DxgiFormat::R32_SINT => vk::Format::R32_SINT,
        DxgiFormat::R16_TYPELESS => vk::Format::R16_UINT,
        DxgiFormat::R16_FLOAT => vk::Format::R16_SFLOAT,
        DxgiFormat::D16_UNORM => vk::Format::D16_UNORM,
        DxgiFormat::R16_UNORM => vk::Format::R16_UNORM,
        DxgiFormat::R16_UINT => vk::Format::R16_UINT,
        DxgiFormat::R16_SNORM => vk::Format::R16_SNORM,
        DxgiFormat::R16_SINT => vk::Format::R16_SINT,
        DxgiFormat::R8_TYPELESS => vk::Format::R8_UNORM,
        DxgiFormat::R8_UNORM => vk::Format::R8_UNORM,
        DxgiFormat::R8_UINT => vk::Format::R8_UINT,
        DxgiFormat::R8_SNORM => vk::Format::R8_SNORM,
        DxgiFormat::R8_SINT => vk::Format::R8_SINT,
        DxgiFormat::A8_UNORM => vk::Format::A8_UNORM_KHR,
        DxgiFormat::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_UNORM,
        DxgiFormat::B8G8R8X8_UNORM => vk::Format::B8G8R8A8_UNORM,
        DxgiFormat::B8G8R8A8_TYPELESS => vk::Format::B8G8R8A8_UNORM,
        DxgiFormat::B8G8R8A8_UNORM_SRGB => vk::Format::B8G8R8A8_SRGB,
        DxgiFormat::B8G8R8X8_TYPELESS => vk::Format::B8G8R8A8_UNORM,
        DxgiFormat::B8G8R8X8_UNORM_SRGB => vk::Format::B8G8R8A8_SRGB,
        DxgiFormat::R9G9B9E5_SHAREDEXP => vk::Format::E5B9G9R9_UFLOAT_PACK32,
        DxgiFormat::B5G6R5_UNORM => vk::Format::R5G6B5_UNORM_PACK16,
        DxgiFormat::B5G5R5A1_UNORM => vk::Format::A1R5G5B5_UNORM_PACK16,
        DxgiFormat::BC1_TYPELESS => vk::Format::BC1_RGBA_UNORM_BLOCK,
        DxgiFormat::BC1_UNORM => vk::Format::BC1_RGBA_UNORM_BLOCK,
        DxgiFormat::BC1_UNORM_SRGB => vk::Format::BC1_RGBA_SRGB_BLOCK,
        DxgiFormat::BC2_TYPELESS => vk::Format::BC2_UNORM_BLOCK,
        DxgiFormat::BC2_UNORM => vk::Format::BC2_UNORM_BLOCK,
        DxgiFormat::BC2_UNORM_SRGB => vk::Format::BC2_SRGB_BLOCK,
        DxgiFormat::BC3_TYPELESS => vk::Format::BC3_UNORM_BLOCK,
        DxgiFormat::BC3_UNORM => vk::Format::BC3_UNORM_BLOCK,
        DxgiFormat::BC3_UNORM_SRGB => vk::Format::BC3_SRGB_BLOCK,
        DxgiFormat::BC4_TYPELESS => vk::Format::BC4_UNORM_BLOCK,
        DxgiFormat::BC4_UNORM => vk::Format::BC4_UNORM_BLOCK,
        DxgiFormat::BC4_SNORM => vk::Format::BC4_SNORM_BLOCK,
        DxgiFormat::BC5_TYPELESS => vk::Format::BC5_UNORM_BLOCK,
        DxgiFormat::BC5_UNORM => vk::Format::BC5_UNORM_BLOCK,
        DxgiFormat::BC5_SNORM => vk::Format::BC5_SNORM_BLOCK,
        DxgiFormat::BC6H_TYPELESS => vk::Format::BC6H_UFLOAT_BLOCK,
        DxgiFormat::BC6H_UF16 => vk::Format::BC6H_UFLOAT_BLOCK,
        DxgiFormat::BC6H_SF16 => vk::Format::BC6H_SFLOAT_BLOCK,
        DxgiFormat::BC7_TYPELESS => vk::Format::BC7_UNORM_BLOCK,
        DxgiFormat::BC7_UNORM => vk::Format::BC7_UNORM_BLOCK,
        DxgiFormat::BC7_UNORM_SRGB => vk::Format::BC7_SRGB_BLOCK,
        DxgiFormat::B4G4R4A4_UNORM => vk::Format::A4R4G4B4_UNORM_PACK16,
        DxgiFormat::A4B4G4R4_UNORM => vk::Format::R4G4B4A4_UNORM_PACK16,
        DxgiFormat::NV12 => vk::Format::G8_B8R8_2PLANE_420_UNORM,
        DxgiFormat::P010 => vk::Format::G16_B16R16_2PLANE_420_UNORM,
        DxgiFormat::P016 => vk::Format::G16_B16R16_2PLANE_420_UNORM,
        DxgiFormat::SAMPLER_FEEDBACK_MIN_MIP_OPAQUE => vk::Format::R32G32_UINT,
        DxgiFormat::SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE => vk::Format::R32G32_UINT,
        _ => return None,
    })
}
