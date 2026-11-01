use std::sync::Arc;

use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{
    ShaderStage,
    scope::{SScope, SScopeStage},
};
use glam::Vec4;
use parking_lot::RwLock;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    asset::AssetManager,
    gpu::{Gpu, command_list::CommandList},
    tfx::dynamic_core::{DynamicCore, DynamicCoreResources},
};

pub struct Scope {
    data: SScope,
    // root_signature: d3d12::RootSignature,
    stage_vertex: ScopeStage,
    stage_pixel: ScopeStage,
}

impl Scope {
    pub fn load(
        gpu: &Gpu,
        asset_manager: &Arc<AssetManager>,
        hash: TagHash,
    ) -> anyhow::Result<Self> {
        let data: SScope = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read scope data")?;

        Ok(Self {
            stage_vertex: ScopeStage::new(
                gpu,
                asset_manager,
                data.stage_vertex.clone(),
                ShaderStage::Vertex,
            )
            .context("while loading vertex stage")?,
            stage_pixel: ScopeStage::new(
                gpu,
                asset_manager,
                data.stage_pixel.clone(),
                ShaderStage::Pixel,
            )
            .context("while loading pixel stage")?,
            data,
        })
    }

    pub fn bind(&self, cmd: &mut CommandList) {
        let _span = trace_span!("Scope::bind", name = ?self.data.name).entered();
        for stage in self.all_stages() {
            stage.bind(cmd);
        }
    }

    pub const fn all_stages(&self) -> [&ScopeStage; 2] {
        [&self.stage_vertex, &self.stage_pixel]
    }

    pub fn stage_by_visibility(&self, visibility: vk::ShaderStageFlags) -> Option<&ScopeStage> {
        self.all_stages()
            .into_iter()
            .find(|stage| stage.visibility == visibility)
    }

    pub fn write_initial_constants(
        &self,
        // cmd: &mut CommandList,
        new_constants: &[Vec4],
    ) -> anyhow::Result<()> {
        for stage in self.all_stages() {
            stage.write_initial_constants(new_constants)?;
        }

        Ok(())
    }
}

pub struct ScopeStage {
    pub core: RwLock<DynamicCore>,
    pub visibility: vk::ShaderStageFlags,
}

impl ScopeStage {
    pub fn new(
        gpu: &Gpu,
        asset_manager: &Arc<AssetManager>,
        stage: SScopeStage,
        shader_stage: ShaderStage,
    ) -> anyhow::Result<Self> {
        let core = DynamicCore::new(gpu, asset_manager, stage.core, shader_stage)?;
        Ok(Self {
            core: RwLock::new(core),
            visibility: shader_stage.vk_shader_stage_flags(),
        })
    }

    pub fn bind(&self, cmd: &mut CommandList) {
        if let Err(e) = self.core.read().prepare(cmd, None) {
            error!("Failed to prepare technique: {}", e);
        }
    }

    /// Overrides the initial constants with the given values.
    ///
    /// Returns an error if the given slice is bigger than the cbuffer. If the slice is smaller, the remaining values are left unchanged.
    pub fn write_initial_constants(
        &self,
        // cmd: &mut CommandList,
        new_constants: &[Vec4],
    ) -> anyhow::Result<()> {
        let cbuffer_size = self.core.read().cbuffer_size / 16;
        if cbuffer_size == 0 {
            // Not all stages have a cbuffer
            return Ok(());
        }

        let mut constants = self.core.write();
        {
            let initial_constants = &mut constants.initial_constants;
            if initial_constants.len() < cbuffer_size {
                initial_constants.resize(cbuffer_size, Vec4::ZERO);
            }

            if new_constants.len() > initial_constants.len() {
                return Err(anyhow::anyhow!("Given slice is bigger than the cbuffer"));
            }
            initial_constants[..new_constants.len()].copy_from_slice(new_constants);
        }

        // // Initial constants aren't normally copied to the cbuffer unless there's expression bytecode, so we need to copy them manually
        // if (constants.bytecode.is_empty() || !constants.writes_cbuffer)
        //     && let Some(cbuffer) = constants.cbuffer.as_ref()
        // {
        //     unsafe {
        //         cbuffer
        //             .write_array(cmd, new_constants)
        //             .expect("Failed to write new initial constants to cbuffer");
        //     }
        // }

        Ok(())
    }
}

pub struct ScopeSamplers {
    stage_vertex: ScopeStageSamplers,
    stage_pixel: ScopeStageSamplers,
}

impl ScopeSamplers {
    pub fn load(gpu: &Gpu, hash: TagHash) -> anyhow::Result<Self> {
        let data: SScope = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read scope data")?;

        Ok(Self {
            stage_vertex: ScopeStageSamplers::new(
                gpu,
                data.stage_vertex.clone(),
                ShaderStage::Vertex,
            )
            .context("while loading vertex stage")?,
            stage_pixel: ScopeStageSamplers::new(gpu, data.stage_pixel.clone(), ShaderStage::Pixel)
                .context("while loading pixel stage")?,
        })
    }

    pub const fn all_stages(&self) -> [&ScopeStageSamplers; 2] {
        [&self.stage_vertex, &self.stage_pixel]
    }

    pub fn stage_by_visibility(
        &self,
        visibility: vk::ShaderStageFlags,
    ) -> Option<&ScopeStageSamplers> {
        self.all_stages()
            .into_iter()
            .find(|stage| stage.visibility == visibility)
    }
}

pub struct ScopeStageSamplers {
    pub core: DynamicCoreResources,
    visibility: vk::ShaderStageFlags,
}

impl ScopeStageSamplers {
    pub fn new(gpu: &Gpu, stage: SScopeStage, shader_stage: ShaderStage) -> anyhow::Result<Self> {
        let core = DynamicCoreResources::extract(gpu, &stage.core, shader_stage)?;
        Ok(Self {
            core,
            visibility: shader_stage.vk_shader_stage_flags(),
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct FrameScope {
    pub game_time: f32,
    pub render_time: f32,
    pub delta_game_time: f32,
    pub exposure_time: f32,

    pub exposure_scale: f32,
    pub exposure_illum_relative_glow: f32,
    pub exposure_scale_for_shading: f32,
    pub exposure_illum_relative: f32,

    pub random_seed_scales: Vec4,
    pub unk3: Vec4,
    pub unk4: Vec4,
    pub unk5: Vec4,
    pub unk6: Vec4,
}

impl FrameScope {
    pub fn to_array(&self) -> &[Vec4; 7] {
        bytemuck::cast_ref::<Self, [Vec4; _]>(self)
    }
}

pub trait ShaderStageExt {
    fn vk_shader_stage_flags(&self) -> vk::ShaderStageFlags;
}

impl ShaderStageExt for ShaderStage {
    fn vk_shader_stage_flags(&self) -> vk::ShaderStageFlags {
        match self {
            Self::Pixel => vk::ShaderStageFlags::FRAGMENT,
            Self::Vertex => vk::ShaderStageFlags::VERTEX,
            Self::Geometry => vk::ShaderStageFlags::GEOMETRY,
            Self::Hull => vk::ShaderStageFlags::TESSELLATION_CONTROL,
            Self::Compute => vk::ShaderStageFlags::COMPUTE,
            Self::Domain => vk::ShaderStageFlags::TESSELLATION_EVALUATION,
        }
    }
}
