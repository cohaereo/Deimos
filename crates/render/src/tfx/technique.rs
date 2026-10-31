use std::sync::Arc;

use ahash::AHashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{STechnique, STechniqueStage, ShaderStage};
use deimos_ecs::object::ObjectChannel;
use smallvec::SmallVec;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    asset::AssetManager,
    gpu::{Gpu, command_list::CommandList},
    tfx::{dynamic_core::DynamicCore, scope::ShaderStageExt},
};

pub struct Technique {
    gpu: Arc<Gpu>,
    pub tag: TagHash,
    pub data: STechnique,
    stage_vertex: TechniqueStage,
    stage_pixel: TechniqueStage,
}

impl Technique {
    pub fn load(
        asset_manager: &Arc<AssetManager>,
        gpu: &Arc<Gpu>,
        hash: TagHash,
    ) -> anyhow::Result<Box<Self>> {
        let data: STechnique = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read technique data")?;

        let mut stage_vertex = TechniqueStage::new(
            asset_manager,
            gpu,
            &data,
            data.shader_vertex.clone(),
            ShaderStage::Vertex,
        )
        .context("while loading vertex stage")?;

        let mut stage_pixel = TechniqueStage::new(
            asset_manager,
            gpu,
            &data,
            data.shader_pixel.clone(),
            ShaderStage::Pixel,
        )
        .context("while loading pixel stage")?;

        Ok(Box::new(Self {
            gpu: gpu.clone(),
            tag: hash,
            data,
            stage_vertex,
            stage_pixel,
        }))
    }

    /// Bind the technique to the command list.
    ///
    /// Make sure to set any necessary states (input layout, etc.) before calling this method, as these need to be compiled into the PSO
    #[profiling::function]
    pub fn bind(&self, cmd: &mut CommandList) {
        self.bind_with_channels(cmd, None);
    }

    pub fn bind_with_channels(
        &self,
        cmd: &mut CommandList,
        object_channels: Option<&AHashMap<u32, ObjectChannel>>,
    ) {
        // todo!()
    }

    pub const fn all_stages(&self) -> [&TechniqueStage; 2] {
        [&self.stage_vertex, &self.stage_pixel]
    }

    pub fn is_loaded(&self) -> bool {
        self.all_stages()
            .iter()
            .all(|s| s.core.all_textures_loaded())
    }
}

pub struct TechniqueStage {
    pub core: DynamicCore,
    pub stage: vk::ShaderStageFlags,

    /// Does this stage have any textures bound outside of the technique's expressions?
    pub has_manual_textures: bool,
    // pub srv_root_param_index: u32,
    // pub descriptor_count: usize,
}

pub struct TechniqueCbufferSlot {
    pub register: u8,
    pub rs_slot: u8,
}

pub struct TechniqueResourceSlot {
    pub register: u8,
    pub descriptor_offset: u8,
    _is_static: bool,
}

impl TechniqueStage {
    pub fn new(
        asset_manager: &Arc<AssetManager>,
        gpu: &Arc<Gpu>,
        // rsb: &mut d3d12::RootSignatureBuilder<'_>,
        technique_data: &STechnique,
        stage: STechniqueStage,
        shader_stage: ShaderStage,
    ) -> anyhow::Result<Self> {
        // let resources = if let Ok(bytecode) =
        //     gpu.pipeline_cache.lock().get_or_load_bytecode(stage.shader)
        //     && let Some(parsed_resources) = d3d12::ext::parse_psv0_resources(&bytecode)
        // {
        //     parsed_resources
        // } else {
        //     vec![]
        // };

        let core = DynamicCore::new(gpu, asset_manager, stage.core, shader_stage)?;

        let mut has_manual_textures = false;
        // for res in &resources {
        //     if matches!(
        //         res.res_type,
        //         PsvResourceType::SRVTyped
        //             | PsvResourceType::SRVRaw
        //             | PsvResourceType::SRVStructured,
        //     ) && !core
        //         .textures
        //         .iter()
        //         .any(|(slot, _)| *slot == res.lower_bound)
        //     {
        //         has_manual_textures = true;
        //         break;
        //     }
        // }

        // for scopes in technique_data.used_scopes.iter() {
        //     let index = scopes.bits().ilog2();
        //     let scope = get_scope_samplers(index);
        //     if let Some(scope_stage) = scope.stage_by_visibility(shader_stage.shader_visibility()) {
        //         for sampler in &scope_stage.core.samplers {
        //             rsb.add_sampler(sampler.clone());
        //         }
        //     }
        // }

        // for sampler in &core.samplers {
        //     rsb.add_sampler(sampler.clone());
        // }

        // let mut root_cbuffer_slots = SmallVec::new();
        // let mut root_texture_slots = SmallVec::new();

        // let mut descriptor_ranges = vec![];
        // let mut descriptor_offset = 0;
        // for resource in &resources {
        //     match resource.res_type {
        //         d3d12::ext::PsvResourceType::SRVTyped
        //         | d3d12::ext::PsvResourceType::SRVRaw
        //         | d3d12::ext::PsvResourceType::SRVStructured => {
        //             let range = d3d12::DescriptorRange {
        //                 range_type: d3d12::DescriptorRangeType::Srv,
        //                 num_descriptors: 1,
        //                 base_shader_register: resource.lower_bound,
        //                 register_space: 0,
        //                 offset_in_descriptors_from_table_start: descriptor_offset,
        //             };
        //             debug_assert!(descriptor_offset <= 0xff);
        //             debug_assert!(resource.lower_bound <= 0xff);

        //             let is_static = core.textures.iter().any(|(slot, src)| {
        //                 *slot == resource.lower_bound
        //                     && matches!(src, ResolvedTextureSource::Static(_))
        //             });
        //             root_texture_slots.push(TechniqueResourceSlot {
        //                 descriptor_offset: descriptor_offset as u8,
        //                 register: resource.lower_bound as u8,
        //                 _is_static: is_static,
        //             });
        //             descriptor_offset += 1;
        //             descriptor_ranges.push(range);
        //         }
        //         d3d12::ext::PsvResourceType::CBV => {
        //             let rs_index = rsb.add_param(
        //                 d3d12::RootParameter::CbvDescriptor {
        //                     shader_register: resource.lower_bound,
        //                     register_space: 0,
        //                 },
        //                 shader_stage.shader_visibility(),
        //             );
        //             debug_assert!(rs_index <= 0xff);
        //             debug_assert!(resource.lower_bound <= 0xff);
        //             root_cbuffer_slots.push(TechniqueCbufferSlot {
        //                 register: resource.lower_bound as u8,
        //                 rs_slot: rs_index as u8,
        //             });
        //         }
        //         _ => {}
        //     }
        // }

        Ok(Self {
            core,
            stage: shader_stage.vk_shader_stage_flags(),
            has_manual_textures,
        })
    }

    #[profiling::function]
    pub fn bind(
        &self,
        cmd: &mut CommandList,
        full_rebind: bool,
        object_channels: Option<&AHashMap<u32, ObjectChannel>>,
    ) {
        // cmd.gpu().descriptor_buffer.cmd_bind_descriptor_buffers(command_buffer, binding_info);
    }
}
