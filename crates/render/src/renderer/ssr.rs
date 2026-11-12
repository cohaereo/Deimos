use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{FixedFunctionState, ShaderStage};
use glam::{Mat4, Vec4};
use smallvec::SmallVec;
use tiger_pkg::TagHash;

use crate::{
    gpu::{
        Gpu,
        command_list::{CommandList, ConstantBufferView, DepthMode},
        pipeline_cache::{PipelineKey, ShaderInfo, TechniqueLayout},
    },
    tfx::technique::Technique,
};

const SSR_VERTEX_SHADER: TagHash = TagHash(0x87000001);
const SSR_PIXEL_SHADER: TagHash = TagHash(0x87000002);

const SSR_VERTEX_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/ssr.vs.dxil");
const SSR_PIXEL_BYTECODE: &[u8] = include_bytes!("../../builtin/shaders/ssr.ps.dxil");

#[derive(Clone, Copy, Debug)]
pub struct SsrSettings {
    pub enabled: bool,
    pub intensity: f32,
    pub max_distance: f32,
    pub thickness: f32,
    pub max_steps: u32,
    pub smoothness_threshold: f32,
}

impl Default for SsrSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.5,
            max_distance: 100.0,
            thickness: 0.04,
            max_steps: 50,
            smoothness_threshold: 0.5,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SsrConstants {
    pub world_to_camera: Mat4,
    pub camera_to_projective: Mat4,
    pub target_pixel_to_camera: Mat4,
    pub params: Vec4,
    pub params2: Vec4,
    pub resolution: Vec4,
}

impl SsrConstants {
    pub fn new(
        world_to_camera: Mat4,
        camera_to_projective: Mat4,
        target_pixel_to_camera: Mat4,
        resolution: (u32, u32),
        settings: &SsrSettings,
    ) -> Self {
        Self {
            world_to_camera,
            camera_to_projective,
            target_pixel_to_camera,
            params: Vec4::new(
                settings.max_distance,
                settings.thickness,
                settings.smoothness_threshold,
                settings.intensity,
            ),
            params2: Vec4::new(settings.max_steps as f32, 0.0, 0.0, 0.0),
            resolution: Vec4::new(
                resolution.0 as f32,
                resolution.1 as f32,
                1.0 / resolution.0 as f32,
                1.0 / resolution.1 as f32,
            ),
        }
    }
}

pub struct SsrRenderer {
    pub settings: SsrSettings,

    layout: TechniqueLayout,
    vertex_shader: ShaderInfo,
    pixel_shader: ShaderInfo,
    pipeline: vk::Pipeline,
}

impl SsrRenderer {
    pub fn new(gpu: &Arc<Gpu>, output_format: vk::Format) -> anyhow::Result<Self> {
        let (layout, vertex_shader, pixel_shader) = {
            let mut cache = gpu.pipeline_cache.write();
            cache.register_builtin_shader_module(SSR_VERTEX_SHADER, SSR_VERTEX_BYTECODE);
            cache.register_builtin_shader_module(SSR_PIXEL_SHADER, SSR_PIXEL_BYTECODE);

            (
                cache.get_or_create_layout(SSR_VERTEX_SHADER, SSR_PIXEL_SHADER)?,
                cache.get_or_load_bytecode(SSR_VERTEX_SHADER, ShaderStage::Vertex)?,
                cache.get_or_load_bytecode(SSR_PIXEL_SHADER, ShaderStage::Pixel)?,
            )
        };

        let key = PipelineKey {
            vertex_shader: SSR_VERTEX_SHADER,
            pixel_shader: SSR_PIXEL_SHADER,
            depth_mode: DepthMode::Reverse,
            fixed_function_state: FixedFunctionState::new(Some(0), Some(15), Some(1), Some(1)),
            topology: vk::PrimitiveTopology::TRIANGLE_STRIP,
            input_layout: 0,
            color_formats: SmallVec::from_slice(&[output_format]),
            depth_format: None,
        };

        let pipeline = gpu.pipeline_cache.write().get_or_create(&key)?;

        Ok(Self {
            settings: SsrSettings::default(),
            layout,
            vertex_shader,
            pixel_shader,
            pipeline,
        })
    }

    pub fn submit(
        &self,
        cmd: &mut CommandList,
        constants: &SsrConstants,
        scene_color: vk::ImageView,
        normal: vk::ImageView,
        depth: vk::ImageView,
        material: vk::ImageView,
    ) -> anyhow::Result<()> {
        let gpu = cmd.gpu().clone();

        let upload = gpu
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(constants))
            .context("Failed to upload SSR constants (UploadHeap OOM)")?;

        cmd.set_shader_constant_buffer_view(
            ShaderStage::Pixel,
            0,
            Some(ConstantBufferView {
                addr: upload.virtual_address(),
                size: upload.len() as u64,
            }),
        );
        cmd.set_shader_resource_view(ShaderStage::Pixel, 0, Some(scene_color));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 1, Some(normal));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 2, Some(depth));
        cmd.set_shader_resource_view(ShaderStage::Pixel, 3, Some(material));
        cmd.set_shader_sampler(ShaderStage::Pixel, 0, Some(gpu.default_sampler));

        let mut offsets = [0u64; 8];
        for set_index in 0..self.layout.set_layouts.len() {
            let size = self.layout.set_sizes[set_index];
            if size == 0 {
                continue;
            }

            let offset = gpu.frame().descriptors.alloc(size)?;
            offsets[set_index] = offset as vk::DeviceSize;

            let (shader_stage, slots) = if set_index < 4 {
                (ShaderStage::Vertex, &self.vertex_shader.slots)
            } else {
                (ShaderStage::Pixel, &self.pixel_shader.slots)
            };

            Technique::write_set(
                &gpu,
                shader_stage,
                slots,
                (set_index % 4) as u32,
                &self.layout.binding_offsets[set_index],
                gpu.frame().descriptors.ptr_at(offset),
                cmd,
            );
        }

        cmd.bind_descriptor_buffer();
        cmd.set_descriptor_buffer_offsets(
            self.layout.pipeline_layout,
            0,
            &self.layout.set_layouts,
            &offsets,
        );
        cmd.bind_pipeline(self.pipeline);
        cmd.draw(0..4, 0..1);

        Ok(())
    }
}
