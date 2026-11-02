use std::sync::Arc;

use anyhow::Context;
use ash::vk;

use crate::gpu::{
    Gpu,
    render_target::{DepthBuffer, RenderTarget},
};

pub struct Gbuffer {
    pub albedo: RenderTarget,
    pub normal: RenderTarget,
    pub normal_read: RenderTarget,
    pub rt3: RenderTarget,
    pub rt4: RenderTarget,

    pub depth: DepthBuffer,
    pub depth_read: DepthBuffer,

    pub uber_depth_half: RenderTarget,

    _resolution: (u32, u32),
}

impl Gbuffer {
    pub fn new(gpu: &Arc<Gpu>, resolution: (u32, u32)) -> anyhow::Result<Self> {
        Ok(Self {
            albedo: RenderTarget::new(
                gpu,
                "deferred_albedo",
                vk::Format::R8G8B8A8_UNORM,
                vk::Format::R8G8B8A8_UNORM,
                resolution,
            )
            .context("allocating render target")?,
            normal: RenderTarget::new(
                gpu,
                "deferred_normal",
                vk::Format::A2B10G10R10_UNORM_PACK32,
                vk::Format::A2B10G10R10_UNORM_PACK32,
                resolution,
            )
            .context("allocating render target")?,
            normal_read: RenderTarget::new(
                gpu,
                "deferred_normal_read",
                vk::Format::A2B10G10R10_UNORM_PACK32,
                vk::Format::A2B10G10R10_UNORM_PACK32,
                resolution,
            )
            .context("allocating render target")?,
            rt3: RenderTarget::new(
                gpu,
                "deferred_rt3",
                vk::Format::R8G8B8A8_UNORM,
                vk::Format::R8G8B8A8_UNORM,
                resolution,
            )
            .context("allocating render target")?,
            rt4: RenderTarget::new(
                gpu,
                "deferred_horizontal_coordinates",
                vk::Format::R32G32_SFLOAT,
                vk::Format::R32G32_SFLOAT,
                resolution,
            )
            .context("allocating render target")?,
            depth: DepthBuffer::new(gpu, resolution).context("allocating depth buffer")?,
            depth_read: DepthBuffer::new(gpu, resolution).context("allocating depth buffer")?,
            uber_depth_half: RenderTarget::new(
                gpu,
                "uber_depth_half",
                vk::Format::R16G16_SFLOAT,
                vk::Format::R16G16_SFLOAT,
                ((resolution.0 / 2).min(1), (resolution.1 / 2).min(1)),
            )
            .context("allocating render target")?,

            _resolution: resolution,
        })
    }

    pub fn clear(&mut self, cmd: &vk::CommandBuffer) {
        self.albedo.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
        self.normal.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
        self.rt3.clear(cmd, [0.0, 0.5, 0.0, 0.0]);
        self.rt4.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
        self.depth.clear(cmd, 0.0, 0xFF);
    }

    pub fn transition(
        &mut self,
        cmd: &vk::CommandBuffer,
        new_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        self.albedo.transition(cmd, new_state);
        self.normal.transition(cmd, new_state);
        self.rt3.transition(cmd, new_state);
        self.rt4.transition(cmd, new_state);
    }
}

pub struct LightBuffer {
    pub light_diffuse: RenderTarget,
    pub light_specular: RenderTarget,
    pub light_specular_ibl: RenderTarget,
    pub distortion: RenderTarget,

    resolution: (u32, u32),
}

impl LightBuffer {
    pub fn new(gpu: &Arc<Gpu>, resolution: (u32, u32)) -> anyhow::Result<Self> {
        Ok(Self {
            light_diffuse: RenderTarget::new(
                gpu,
                "light_diffuse",
                vk::Format::B10G11R11_UFLOAT_PACK32,
                vk::Format::B10G11R11_UFLOAT_PACK32,
                resolution,
            )
            .context("allocating render target")?,
            light_specular: RenderTarget::new(
                gpu,
                "light_specular",
                vk::Format::B10G11R11_UFLOAT_PACK32,
                vk::Format::B10G11R11_UFLOAT_PACK32,
                resolution,
            )
            .context("allocating render target")?,
            light_specular_ibl: RenderTarget::new(
                gpu,
                "light_specular_ibl",
                vk::Format::B10G11R11_UFLOAT_PACK32,
                vk::Format::B10G11R11_UFLOAT_PACK32,
                resolution,
            )
            .context("allocating render target")?,
            distortion: RenderTarget::new(
                gpu,
                "distortion",
                vk::Format::R8G8B8A8_UNORM,
                vk::Format::R8G8B8A8_UNORM,
                resolution,
            )
            .context("allocating render target")?,
            resolution,
        })
    }

    pub fn clear(&mut self, cmd: &vk::CommandBuffer) {
        self.light_diffuse.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
        self.light_specular.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
        self.light_specular_ibl.clear(cmd, [0.0, 0.0, 0.0, 0.0]);
    }

    pub fn transition(
        &mut self,
        cmd: &vk::CommandBuffer,
        new_state: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        self.light_diffuse.transition(cmd, new_state);
        self.light_specular.transition(cmd, new_state);
        self.light_specular_ibl.transition(cmd, new_state);
    }
}
