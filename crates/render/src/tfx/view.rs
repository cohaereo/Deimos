// pub enum View {
//     Shaded(ShadedView),
//     Shadow(ShadowView),
// }

use std::ops::Range;
use std::sync::Arc;

use ash::vk;
use glam::Mat4;

use crate::{
    gpu::{
        Gpu,
        render_target::{DepthBuffer, RenderTarget},
    },
    renderer::cascades::CascadeCalculator,
    tfx::buffers::{Gbuffer, LightBuffer},
    visibility::frustum::Frustum,
};

pub struct ShadedView {
    pub culling_frustum: Frustum,
    pub world_to_camera: Mat4,
    pub camera_to_projective: Mat4,

    pub gbuffer: Gbuffer,
    pub light: LightBuffer,
    pub shaded_read: RenderTarget,

    pub postprocess: RenderTarget,
    pub output: RenderTarget,
    pub shadow_views: [ShadowCascade; CascadeCalculator::MAX_CASCADES],
    pub shadow_mask: RenderTarget,

    resolution: (u32, u32),
    resolution_scale: f32,
}

impl ShadedView {
    pub const MAIN_VIEW_ID: usize = 0;

    pub fn new(gpu: &Arc<Gpu>, resolution: (u32, u32)) -> anyhow::Result<Self> {
        Ok(Self {
            culling_frustum: Frustum::default(),
            world_to_camera: Mat4::default(),
            camera_to_projective: Mat4::default(),
            gbuffer: Gbuffer::new(gpu, resolution)?,
            light: LightBuffer::new(gpu, resolution)?,
            postprocess: RenderTarget::new(
                gpu,
                "postprocess",
                vk::Format::R16G16B16A16_SFLOAT,
                vk::Format::R16G16B16A16_SFLOAT,
                resolution,
            )?,
            shaded_read: RenderTarget::new(
                gpu,
                "shaded_read",
                vk::Format::B10G11R11_UFLOAT_PACK32,
                vk::Format::B10G11R11_UFLOAT_PACK32,
                resolution,
            )?,
            output: RenderTarget::new(
                gpu,
                "result",
                vk::Format::B10G11R11_UFLOAT_PACK32,
                vk::Format::B10G11R11_UFLOAT_PACK32,
                resolution,
            )?,
            shadow_views: [
                ShadowCascade::new(gpu)?,
                ShadowCascade::new(gpu)?,
                ShadowCascade::new(gpu)?,
                ShadowCascade::new(gpu)?,
            ],
            shadow_mask: RenderTarget::new(
                gpu,
                "shadow_mask",
                vk::Format::R8G8_UNORM,
                vk::Format::R8G8_UNORM,
                resolution,
            )?,
            resolution,
            resolution_scale: 1.0,
        })
    }

    pub fn resize(&mut self, gpu: &Arc<Gpu>, new_resolution: (u32, u32)) -> anyhow::Result<()> {
        let new_resolution = (
            ((new_resolution.0 as f32 * self.resolution_scale) as u32).max(64),
            ((new_resolution.1 as f32 * self.resolution_scale) as u32).max(64),
        );
        if new_resolution == self.resolution {
            return Ok(());
        }

        let resolution_scale = self.resolution_scale;
        *self = Self::new(gpu, new_resolution)?;
        self.resolution_scale = resolution_scale;
        Ok(())
    }

    pub const fn set_resolution_scale(&mut self, scale: f32) {
        self.resolution_scale = scale;
    }

    pub const fn resolution_scale(&self) -> f32 {
        self.resolution_scale
    }

    pub const fn resolution(&self) -> (u32, u32) {
        self.resolution
    }
}

pub struct ShadowCascade {
    pub id: usize,
    pub depth: DepthBuffer,

    pub world_to_camera: glam::Mat4,
    pub camera_to_projective: glam::Mat4,

    pub depth_range: Range<f32>,

    pub frustum: Frustum,
}

impl ShadowCascade {
    pub fn new(gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        Ok(Self {
            id: 0,
            depth: DepthBuffer::new(
                gpu,
                (
                    CascadeCalculator::SHADOW_MAP_RESOLUTION,
                    CascadeCalculator::SHADOW_MAP_RESOLUTION,
                ),
            )?,
            camera_to_projective: glam::Mat4::IDENTITY,
            world_to_camera: glam::Mat4::IDENTITY,
            depth_range: 0f32..0f32,
            frustum: Frustum::default(),
        })
    }

    pub fn world_to_projective(&self) -> glam::Mat4 {
        self.camera_to_projective * self.world_to_camera
    }
}

pub struct ViewSettings {
    pub exposure_scale: f32,
    pub exposure_illum_relative: f32,
    pub draw_hdri_background: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            exposure_scale: 1.0,
            exposure_illum_relative: 1.0,
            draw_hdri_background: true,
        }
    }
}
