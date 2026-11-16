use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{FixedFunctionState, ShaderStage};
use glam::{Mat4, Vec4};
use tiger_pkg::TagHash;

use crate::{
    gpu::{
        Gpu,
        alloc::{BufferAllocDesc, ImageAllocDesc, image::Image},
        command_buffer::{
            IMAGE_STATE_COMPUTE_READ, IMAGE_STATE_COMPUTE_STORAGE, IMAGE_STATE_SHADER_READ,
            IMAGE_STATE_TRANSFER_DST,
        },
        command_list::{CommandList, ConstantBufferView},
        pipeline_cache::TechniqueLayout,
    },
    renderer::fullscreen_pass::FullscreenPass,
    tfx::technique::Technique,
};

// TODO(cohae): Use a const/lazy auto-incrementing taghash for this instead of hardcoded handles
const PREPASS_SHADER: TagHash = TagHash(0x87000010);
const MAIN_SHADER: TagHash = TagHash(0x87000011);
const FILTER_SHADER: TagHash = TagHash(0x87000012);
const APPLY_VERTEX_SHADER: TagHash = TagHash(0x87000013);
const APPLY_PIXEL_SHADER: TagHash = TagHash(0x87000014);
const TEMPORAL_SHADER: TagHash = TagHash(0x87000015);

const PREPASS_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_prepass.cs.dxil");
const MAIN_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_main.cs.dxil");
const FILTER_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_filter.cs.dxil");
const APPLY_VERTEX_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_apply.vs.dxil");
const APPLY_PIXEL_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_apply.ps.dxil");
const TEMPORAL_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/cortao_temporal.cs.dxil");

const OCCLUDER_TABLE: &[u8] =
    include_bytes!("../../builtin/shaders/cortao/24x24x16x16_R16_UINT.bin");
const OCCLUDER_TABLE_EXTENT: vk::Extent3D = vk::Extent3D {
    width: 24,
    height: 24,
    depth: 16,
};

const PACKED_MIP_COUNT: u32 = 8;

#[derive(Clone, Copy, Debug)]
pub struct CortaoSettings {
    pub enabled: bool,
    pub max_radius: f32,
    pub max_screenspace_radius: f32,
    pub mip_bias: f32,
    pub max_slope_compensation: f32,
    pub backface_threshold: f32,
    pub backface_boost: f32,
    pub depth_sensitivity: f32,
    pub normal_sensitivity: f32,
    pub temporal_alpha: f32,
}

impl Default for CortaoSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_radius: 1.0,
            max_screenspace_radius: 0.2,
            mip_bias: -4.0,
            max_slope_compensation: 4.0,
            backface_threshold: 0.5,
            backface_boost: 0.5,
            depth_sensitivity: 64.0,
            normal_sensitivity: 16.0,
            temporal_alpha: 0.1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PrepassConstants {
    packed_resolution: u32,
    depth_scale: f32,
    depth_offset: f32,
    random_seed: u32,
    mip_count: u32,
    _padding: [u32; 3],
    world_to_camera: Mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MainConstants {
    packed_resolution: u32,
    random_seed: u32,
    normal_bias: f32,
    radius: f32,
    unproject_params: Vec4,
    projection_params: Vec4,
    max_apparent_circle_radius_coefficient: f32,
    mip_bias: f32,
    inverse_max_slope_compensation: f32,
    backface_threshold: f32,
    backface_boost: f32,
    _padding: [u32; 3],
    world_to_camera: Mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FilterConstants {
    packed_resolution: u32,
    depth_sensitivity: f32,
    normal_sensitivity: f32,
    _padding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TemporalConstants {
    packed_resolution: u32,
    temporal_alpha: f32,
    depth_reject_threshold: f32,
    _padding0: u32,
    unproject_params: Vec4,
    cur_inv_world_to_camera: Mat4,
    prev_world_to_camera: Mat4,
    prev_projection_params: Vec4,
}

struct CortaoPass {
    pipeline: vk::Pipeline,
    layout: TechniqueLayout,
    slots: dxil2spirv::SlotUsage,
}

impl CortaoPass {
    fn new(gpu: &Arc<Gpu>, shader: TagHash) -> anyhow::Result<Self> {
        let (pipeline, layout, slots) = {
            let mut cache = gpu.pipeline_cache.write();
            let (pipeline, layout) = cache.get_or_create_compute(shader)?;
            let info = cache.shader_info_compute(shader)?;
            (pipeline, layout, info.slots)
        };

        Ok(Self {
            pipeline,
            layout,
            slots,
        })
    }

    fn dispatch(
        &self,
        gpu: &Arc<Gpu>,
        cmd: &mut CommandList,
        constants: ConstantBufferView,
        group_counts: [u32; 3],
    ) -> anyhow::Result<()> {
        cmd.set_shader_constant_buffer_view(ShaderStage::Compute, 0, Some(constants));

        let mut offsets = [0u64; 8];
        for set_index in 0..self.layout.set_layouts.len() {
            let size = self.layout.set_sizes[set_index];
            if size == 0 {
                continue;
            }

            let offset = gpu.frame().descriptors.alloc(size)?;
            offsets[set_index] = offset as vk::DeviceSize;

            Technique::write_set(
                gpu,
                ShaderStage::Compute,
                &self.slots,
                set_index as u32,
                &self.layout.binding_offsets[set_index],
                gpu.frame().descriptors.ptr_at(offset),
                cmd,
            );
        }

        cmd.bind_descriptor_buffer();
        cmd.set_compute_descriptor_buffer_offsets(
            self.layout.pipeline_layout,
            0,
            &self.layout.set_layouts,
            &offsets,
        );
        cmd.bind_compute_pipeline(self.pipeline);
        cmd.dispatch(group_counts);

        Ok(())
    }
}

struct StorageImage {
    image: Image,
    view: vk::ImageView,
}

impl StorageImage {
    fn new(
        gpu: &Arc<Gpu>,
        name: &str,
        format: vk::Format,
        (width, height): (u32, u32),
    ) -> anyhow::Result<Self> {
        let image = gpu.alloc_image(&ImageAllocDesc {
            name,
            image_type: vk::ImageType::TYPE_2D,
            format,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_DST,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            location: gpu_allocator::MemoryLocation::GpuOnly,
        })?;

        let view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        }?;

        Ok(Self { image, view })
    }

    fn clear(&mut self, gpu: &Arc<Gpu>, cmd: &mut CommandList, value: [f32; 4]) {
        self.image.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_TRANSFER_DST,
        );

        unsafe {
            gpu.cmd_clear_color_image(
                cmd.native(),
                self.image.vk_image(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &vk::ClearColorValue { float32: value },
                &[vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .level_count(1)],
            );
        }
    }
}

pub struct CortaoRenderer {
    pub settings: CortaoSettings,

    resolution: (u32, u32),

    prepass: CortaoPass,
    main: CortaoPass,
    filter: CortaoPass,
    temporal: CortaoPass,

    apply: FullscreenPass,

    packed_data: Image,
    packed_views: [vk::ImageView; PACKED_MIP_COUNT as usize],
    packed_full_view: vk::ImageView,

    occlusion: StorageImage,
    filtered: StorageImage,
    history: [StorageImage; 2],
    history_index: usize,
    prev_packed: StorageImage,
    first_frame: bool,
    prev_world_to_camera: Mat4,
    prev_projection_params: Vec4,

    _occluder_table: Image,
    occluder_view: vk::ImageView,

    border_sampler: vk::Sampler,
    clamp_sampler: vk::Sampler,
    linear_clamp_sampler: vk::Sampler,
}

impl CortaoRenderer {
    pub fn new(gpu: &Arc<Gpu>, resolution: (u32, u32)) -> anyhow::Result<Self> {
        let mut cache = gpu.pipeline_cache.write();
        cache.register_builtin_shader_module(PREPASS_SHADER, PREPASS_BYTECODE);
        cache.register_builtin_shader_module(MAIN_SHADER, MAIN_BYTECODE);
        cache.register_builtin_shader_module(FILTER_SHADER, FILTER_BYTECODE);
        cache.register_builtin_shader_module(TEMPORAL_SHADER, TEMPORAL_BYTECODE);
        cache.register_builtin_shader_module(APPLY_VERTEX_SHADER, APPLY_VERTEX_BYTECODE);
        cache.register_builtin_shader_module(APPLY_PIXEL_SHADER, APPLY_PIXEL_BYTECODE);
        drop(cache);

        let prepass = CortaoPass::new(gpu, PREPASS_SHADER)?;
        let main = CortaoPass::new(gpu, MAIN_SHADER)?;
        let filter = CortaoPass::new(gpu, FILTER_SHADER)?;
        let temporal = CortaoPass::new(gpu, TEMPORAL_SHADER)?;
        let apply = FullscreenPass::new(
            gpu,
            APPLY_VERTEX_SHADER,
            APPLY_PIXEL_SHADER,
            APPLY_VERTEX_BYTECODE,
            APPLY_PIXEL_BYTECODE,
        )?;

        let (packed_data, packed_views, packed_full_view) = create_packed_data(gpu, resolution)?;

        let border_sampler = unsafe {
            gpu.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .border_color(vk::BorderColor::FLOAT_TRANSPARENT_BLACK)
                    .max_lod(16.0),
                None,
            )
        }?;

        let clamp_sampler = unsafe {
            gpu.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )
        }?;

        let linear_clamp_sampler = unsafe {
            gpu.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )
        }?;

        let (occluder_table, occluder_view) = load_occluder_table(gpu)?;

        let renderer = Self {
            settings: CortaoSettings::default(),
            resolution,
            prepass,
            main,
            filter,
            temporal,
            apply,
            packed_data,
            packed_views,
            packed_full_view,
            occlusion: StorageImage::new(
                gpu,
                "cortao_occlusion",
                vk::Format::R32_SFLOAT,
                resolution,
            )?,
            filtered: StorageImage::new(
                gpu,
                "cortao_filtered",
                vk::Format::R32_SFLOAT,
                resolution,
            )?,
            history: [
                StorageImage::new(gpu, "cortao_history_a", vk::Format::R16_SFLOAT, resolution)?,
                StorageImage::new(gpu, "cortao_history_b", vk::Format::R16_SFLOAT, resolution)?,
            ],
            history_index: 0,
            prev_packed: StorageImage::new(
                gpu,
                "cortao_prev_packed",
                vk::Format::R32_UINT,
                resolution,
            )?,
            first_frame: true,
            prev_world_to_camera: Mat4::ZERO,
            prev_projection_params: Vec4::ZERO,
            _occluder_table: occluder_table,
            occluder_view,
            border_sampler,
            clamp_sampler,
            linear_clamp_sampler,
        };

        Ok(renderer)
    }

    pub const fn resolution(&self) -> (u32, u32) {
        self.resolution
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        cmd: &mut CommandList,
        depth: vk::ImageView,
        normal: vk::ImageView,
        world_to_camera: Mat4,
        camera_to_projective: Mat4,
        near: f32,
        far: f32,
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        let random_seed = (gpu.frame_index() as u32).wrapping_mul(0x9E3779B9);

        let (width, height) = self.resolution;
        let packed_resolution = width | (height << 16);

        let prepass_constants = PrepassConstants {
            packed_resolution,
            depth_scale: (far - near) / (far * near),
            depth_offset: 1.0 / far,
            random_seed,
            mip_count: PACKED_MIP_COUNT,
            _padding: [0; 3],
            world_to_camera,
        };

        let (unproject_params, projection_params) = projection_params(&camera_to_projective);
        let inv_x = unproject_params.x * 0.5;
        let inv_y = -unproject_params.y * 0.5;

        let max_apparent_circle_radius = inv_y * self.settings.max_screenspace_radius * 2.0;
        let main_constants = MainConstants {
            packed_resolution,
            random_seed,
            normal_bias: inv_x / width as f32 * std::f32::consts::SQRT_2,
            radius: self.settings.max_radius,
            unproject_params,
            projection_params,
            max_apparent_circle_radius_coefficient: max_apparent_circle_radius
                / max_apparent_circle_radius
                    .mul_add(max_apparent_circle_radius, 1.0)
                    .sqrt(),
            mip_bias: (projection_params.y * height as f32 * 0.5).log2() + self.settings.mip_bias,
            inverse_max_slope_compensation: 1.0 / self.settings.max_slope_compensation,
            backface_threshold: self.settings.backface_threshold,
            backface_boost: self.settings.backface_boost,
            _padding: [0; 3],
            world_to_camera,
        };

        let filter_constants = FilterConstants {
            packed_resolution,
            depth_sensitivity: self.settings.depth_sensitivity,
            normal_sensitivity: self.settings.normal_sensitivity,
            _padding: 0,
        };

        // First frame has no history to blend with
        let temporal_alpha = if self.first_frame {
            1.0
        } else {
            self.settings.temporal_alpha
        };
        self.first_frame = false;

        let temporal_constants = TemporalConstants {
            packed_resolution,
            temporal_alpha,
            depth_reject_threshold: 0.1,
            _padding0: 0,
            unproject_params,
            cur_inv_world_to_camera: world_to_camera.inverse(),
            prev_world_to_camera: self.prev_world_to_camera,
            prev_projection_params: self.prev_projection_params,
        };

        let prepass_upload = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&prepass_constants))
            .context("Failed to upload CORTAO prepass constants")?;
        let main_upload = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&main_constants))
            .context("Failed to upload CORTAO main pass constants")?;
        let filter_upload = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&filter_constants))
            .context("Failed to upload CORTAO filter constants")?;
        let temporal_upload = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&temporal_constants))
            .context("Failed to upload CORTAO temporal constants")?;
        let packed_view = self.packed_full_view;
        let packed_mips = self.packed_views;
        let occluder_view = self.occluder_view;
        let convergence = [width.div_ceil(8), height.div_ceil(8), 1];

        self.packed_data.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_STORAGE,
        );
        {
            let _scope = gpu.profiler_scope(cmd, "cortao_prepass");

            cmd.set_shader_resource_view(ShaderStage::Compute, 0, Some(depth));
            cmd.set_shader_resource_view(ShaderStage::Compute, 1, Some(normal));
            for (mip, view) in packed_mips.iter().enumerate() {
                cmd.set_unordered_access_view(ShaderStage::Compute, mip as u32, Some(*view));
            }

            self.prepass.dispatch(
                &gpu,
                cmd,
                ConstantBufferView {
                    addr: prepass_upload.virtual_address(),
                    size: prepass_upload.len() as u64,
                },
                convergence,
            )?;
        }
        self.packed_data.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_READ,
        );
        self.occlusion.clear(&gpu, cmd, [1.0; 4]);
        self.occlusion.image.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_STORAGE,
        );
        {
            let _scope = gpu.profiler_scope(cmd, "cortao_main");
            cmd.set_shader_resource_view(ShaderStage::Compute, 0, Some(packed_view));
            cmd.set_shader_resource_view(ShaderStage::Compute, 1, Some(normal));
            cmd.set_shader_resource_view(ShaderStage::Compute, 2, Some(occluder_view));
            cmd.set_shader_sampler(ShaderStage::Compute, 0, Some(self.border_sampler));
            cmd.set_shader_sampler(ShaderStage::Compute, 1, Some(self.clamp_sampler));
            cmd.set_unordered_access_view(ShaderStage::Compute, 0, Some(self.occlusion.view));

            self.main.dispatch(
                &gpu,
                cmd,
                ConstantBufferView {
                    addr: main_upload.virtual_address(),
                    size: main_upload.len() as u64,
                },
                [width.div_ceil(16), height.div_ceil(16), 1],
            )?;
        }
        self.occlusion.image.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_READ,
        );

        self.filtered.clear(&gpu, cmd, [1.0; 4]);
        self.filtered.image.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_STORAGE,
        );
        {
            let _scope = gpu.profiler_scope(cmd, "cortao_filter");
            cmd.set_shader_resource_view(ShaderStage::Compute, 0, Some(packed_view));
            cmd.set_shader_resource_view(ShaderStage::Compute, 1, Some(self.occlusion.view));
            cmd.set_shader_sampler(ShaderStage::Compute, 0, Some(self.border_sampler));
            cmd.set_unordered_access_view(ShaderStage::Compute, 0, Some(self.filtered.view));

            self.filter.dispatch(
                &gpu,
                cmd,
                ConstantBufferView {
                    addr: filter_upload.virtual_address(),
                    size: filter_upload.len() as u64,
                },
                [width.div_ceil(16), height.div_ceil(16), 1],
            )?;
        }
        self.filtered.image.layout_barrier(
            cmd.native(),
            vk::ImageAspectFlags::COLOR,
            IMAGE_STATE_COMPUTE_READ,
        );
        {
            let _scope = gpu.profiler_scope(cmd, "cortao_temporal");
            let prev_packed_view = self.prev_packed.view;
            let read_index = self.history_index;
            let write_index = 1 - read_index;
            let history_read_view = self.history[read_index].view;
            let history_write_view = self.history[write_index].view;
            self.history[write_index].image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_STORAGE,
            );
            self.history[read_index].image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_READ,
            );
            self.prev_packed.image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_READ,
            );
            cmd.set_shader_resource_view(ShaderStage::Compute, 0, Some(self.filtered.view));
            cmd.set_shader_resource_view(ShaderStage::Compute, 1, Some(packed_view));
            cmd.set_shader_resource_view(ShaderStage::Compute, 2, Some(prev_packed_view));
            cmd.set_shader_resource_view(ShaderStage::Compute, 3, Some(history_read_view));
            cmd.set_shader_sampler(ShaderStage::Compute, 0, Some(self.clamp_sampler));
            cmd.set_shader_sampler(ShaderStage::Compute, 1, Some(self.border_sampler));
            cmd.set_shader_sampler(ShaderStage::Compute, 2, Some(self.linear_clamp_sampler));
            cmd.set_unordered_access_view(ShaderStage::Compute, 0, Some(history_write_view));

            self.temporal.dispatch(
                &gpu,
                cmd,
                ConstantBufferView {
                    addr: temporal_upload.virtual_address(),
                    size: temporal_upload.len() as u64,
                },
                [width.div_ceil(8), height.div_ceil(8), 1],
            )?;
            self.history[write_index].image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_SHADER_READ,
            );
            self.history_index = write_index;
        }

        {
            let _scope = gpu.profiler_scope(cmd, "cortao_prev_depth_copy");
            self.packed_data.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_STORAGE,
            );
            self.prev_packed.image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_STORAGE,
            );
            cmd.copy_image(
                &self.packed_data,
                &self.prev_packed.image,
                &[vk::ImageCopy::default()
                    .src_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .layer_count(1),
                    )
                    .dst_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .layer_count(1),
                    )
                    .extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    })],
            );
            self.prev_packed.image.layout_barrier(
                cmd.native(),
                vk::ImageAspectFlags::COLOR,
                IMAGE_STATE_COMPUTE_READ,
            );
        }

        self.prev_world_to_camera = world_to_camera;
        self.prev_projection_params = projection_params;

        Ok(())
    }

    pub const fn occlusion_view(&self) -> vk::ImageView {
        self.history[self.history_index].view
    }

    pub fn apply(&self, cmd: &mut CommandList, scene_color: vk::ImageView) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();
        let fixed_function_state = apply_ffstate();

        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(scene_color));
        cmd.set_shader_resource_view(
            ShaderStage::Pixel,
            1,
            Some(self.history[self.history_index].view),
        );

        let pipeline = self.apply.get_pipeline(&gpu, cmd, fixed_function_state)?;
        self.apply.bind(&gpu, cmd, pipeline)?;
        cmd.draw(0..4, 0..1);

        Ok(())
    }
}

fn projection_params(camera_to_projective: &Mat4) -> (Vec4, Vec4) {
    let inverse = camera_to_projective.inverse();
    let (inv_x, inv_y) = (inverse.x_axis.x, inverse.y_axis.y);
    let (inv_offset_x, inv_offset_y) = (inverse.w_axis.x, inverse.w_axis.y);
    let (proj_x, proj_y) = (camera_to_projective.x_axis.x, camera_to_projective.y_axis.y);
    let (proj_offset_x, proj_offset_y) =
        (camera_to_projective.z_axis.x, camera_to_projective.z_axis.y);

    (
        Vec4::new(
            2.0 * inv_x,
            -2.0 * inv_y,
            -inv_x + inv_offset_x,
            inv_y + inv_offset_y,
        ),
        Vec4::new(
            0.5 * proj_x,
            -0.5 * proj_y,
            0.5f32.mul_add(proj_offset_x, -0.5),
            0.5f32.mul_add(-proj_offset_y, -0.5),
        ),
    )
}

pub fn apply_ffstate() -> FixedFunctionState {
    FixedFunctionState::new(Some(0), Some(15), Some(1), Some(1))
}

fn create_packed_data(
    gpu: &Arc<Gpu>,
    (width, height): (u32, u32),
) -> anyhow::Result<(
    Image,
    [vk::ImageView; PACKED_MIP_COUNT as usize],
    vk::ImageView,
)> {
    let image = gpu.alloc_image(&ImageAllocDesc {
        name: "cortao_packed_data",
        image_type: vk::ImageType::TYPE_2D,
        format: vk::Format::R32_UINT,
        extent: vk::Extent3D {
            width,
            height,
            depth: 1,
        },
        mip_levels: PACKED_MIP_COUNT,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage: vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::TRANSFER_SRC,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        location: gpu_allocator::MemoryLocation::GpuOnly,
    })?;

    let full_view = unsafe {
        gpu.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image.vk_image())
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R32_UINT)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1)
                        .level_count(PACKED_MIP_COUNT),
                ),
            None,
        )
    }?;

    let mut mip_views = [vk::ImageView::null(); PACKED_MIP_COUNT as usize];
    for (mip, view) in mip_views.iter_mut().enumerate() {
        *view = unsafe {
            gpu.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R32_UINT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(mip as u32)
                            .level_count(1)
                            .layer_count(1),
                    ),
                None,
            )
        }?;
    }

    Ok((image, mip_views, full_view))
}

fn load_occluder_table(gpu: &Arc<Gpu>) -> anyhow::Result<(Image, vk::ImageView)> {
    let staging = gpu.alloc_buffer(&BufferAllocDesc {
        name: "cortao_occluder_staging",
        size: OCCLUDER_TABLE.len() as u64,
        usage: vk::BufferUsageFlags::TRANSFER_SRC,
        flags: vk::BufferCreateFlags::empty(),
        location: gpu_allocator::MemoryLocation::CpuToGpu,
    })?;

    unsafe {
        std::ptr::copy_nonoverlapping(
            OCCLUDER_TABLE.as_ptr(),
            staging.mapped_ptr().as_ptr(),
            OCCLUDER_TABLE.len(),
        );
    }

    let mut image = gpu.alloc_image(&ImageAllocDesc {
        name: "cortao_occluder",
        image_type: vk::ImageType::TYPE_3D,
        format: vk::Format::R16_UINT,
        extent: OCCLUDER_TABLE_EXTENT,
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        location: gpu_allocator::MemoryLocation::GpuOnly,
    })?;

    let cmd = gpu.upload_command_buffer();
    image.layout_barrier(*cmd, vk::ImageAspectFlags::COLOR, IMAGE_STATE_TRANSFER_DST);

    let region = vk::BufferImageCopy::default()
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1),
        )
        .image_extent(OCCLUDER_TABLE_EXTENT);

    unsafe {
        gpu.cmd_copy_buffer_to_image(
            *cmd,
            staging.vk_buffer(),
            image.vk_image(),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            std::slice::from_ref(&region),
        );
    }

    image.layout_barrier(*cmd, vk::ImageAspectFlags::COLOR, IMAGE_STATE_SHADER_READ);

    let view = unsafe {
        gpu.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image.vk_image())
                .view_type(vk::ImageViewType::TYPE_3D)
                .format(vk::Format::R16_UINT)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1)
                        .level_count(1),
                ),
            None,
        )
    }?;

    Ok((image, view))
}
