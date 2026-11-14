use std::sync::Arc;

use ahash::AHashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{STechnique, STechniqueStage, ShaderStage, TechniqueBindMode};
use deimos_ecs::object::ObjectChannel;
use dxil2spirv::{SlotType, SlotUsage};
use parking_lot::RwLock;
use smallvec::SmallVec;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    asset::AssetManager,
    gpu::{
        Gpu,
        command_list::{CommandList, ConstantBufferView},
        descriptor::DescriptorWriter,
        pipeline_cache::{PipelineKey, ShaderInfo, TechniqueLayout},
    },
    tfx::{dynamic_core::DynamicCore, scope::ShaderStageExt},
};

const MAX_TECHNIQUE_LOCAL_PIPELINES: usize = 16;

pub struct Technique {
    pub tag: TagHash,
    pub data: STechnique,
    pub layout: TechniqueLayout,
    pub stage_vertex: TechniqueStage,
    pub stage_pixel: TechniqueStage,
    pipeline_cache: RwLock<SmallVec<[(PipelineKey, vk::Pipeline); 4]>>,
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

        let stage_vertex = TechniqueStage::new(
            asset_manager,
            gpu,
            &data,
            data.shader_vertex.clone(),
            ShaderStage::Vertex,
        )
        .context("while loading vertex stage")?;

        let stage_pixel = TechniqueStage::new(
            asset_manager,
            gpu,
            &data,
            data.shader_pixel.clone(),
            ShaderStage::Pixel,
        )
        .context("while loading pixel stage")?;

        let layout = gpu
            .pipeline_cache
            .write()
            .get_or_create_layout(data.shader_vertex.shader, data.shader_pixel.shader)
            .context("while creating technique descriptor layout")?;

        Ok(Box::new(Self {
            tag: hash,
            data,
            layout,
            stage_vertex,
            stage_pixel,
            pipeline_cache: RwLock::new(SmallVec::new()),
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
        let _span = trace_span!("Technique::bind", tag = %self.tag).entered();

        if !matches!(
            self.data.bind_mode,
            TechniqueBindMode::VertexOnly | TechniqueBindMode::VertexPixel
        ) {
            error!("{:?} bind mode not implemented", self.data.bind_mode);
            return;
        }

        for stage in self.all_stages() {
            if let Err(e) = stage.core.prepare(cmd, object_channels) {
                error!("Failed to prepare technique stage: {}", e);
                return;
            }
        }

        let gpu = cmd.gpu().clone();
        let fixed_function_state = cmd
            .cmd_state()
            .ffstate
            .select(&self.data.states)
            .select(&cmd.cmd_state().ffstate_override);

        let pipeline_key = PipelineKey {
            vertex_shader: self.data.shader_vertex.shader,
            pixel_shader: cmd
                .pixel_shader_override
                .unwrap_or(self.data.shader_pixel.shader),
            depth_mode: cmd.depth_mode(),
            fixed_function_state,
            topology: cmd.current_topology(),
            input_layout: cmd.get_input_layout() as u8,
            color_formats: SmallVec::from_slice(cmd.color_attachment_formats()),
            depth_format: cmd.depth_attachment_format(),
        };

        let layout = &self.layout;

        let cached_pipeline = {
            let cache = self.pipeline_cache.read();
            cache
                .iter()
                .find(|(key, _)| key == &pipeline_key)
                .map(|(_, pipeline)| *pipeline)
        };
        let pipeline = if let Some(pipeline) = cached_pipeline {
            pipeline
        } else {
            let existing_pipeline = { gpu.pipeline_cache.read().get(&pipeline_key) };
            let pipeline = if let Some(pipeline) = existing_pipeline {
                pipeline
            } else {
                let mut cache = gpu.pipeline_cache.write();
                match cache.get_or_create(&pipeline_key) {
                    Ok(pipeline) => pipeline,
                    Err(e) => {
                        error!("Failed to create pipeline: {e:?}");
                        return;
                    }
                }
            };

            let mut cache = self.pipeline_cache.write();
            if !cache.iter().any(|(key, _)| key == &pipeline_key) {
                if cache.len() >= MAX_TECHNIQUE_LOCAL_PIPELINES {
                    cache.clear();
                }
                cache.push((pipeline_key, pipeline));
            }
            pipeline
        };

        let mut offsets = [0u64; 8];
        for set_index in 0..layout.set_layouts.len() {
            let size = layout.set_sizes[set_index];
            if size == 0 {
                continue;
            }

            let offset = match gpu.frame().descriptors.alloc(size) {
                Ok(offset) => offset,
                Err(e) => {
                    error!("Failed to allocate descriptor set: {e:?}");
                    return;
                }
            };
            offsets[set_index] = offset as vk::DeviceSize;

            let stage = if set_index < 4 {
                &self.stage_vertex
            } else {
                &self.stage_pixel
            };
            Self::write_set(
                &gpu,
                stage.shader_stage,
                &stage.shader_info.slots,
                (set_index % 4) as u32,
                &layout.binding_offsets[set_index],
                gpu.frame().descriptors.ptr_at(offset),
                cmd,
            );
        }

        cmd.bind_descriptor_buffer();
        cmd.set_descriptor_buffer_offsets(layout.pipeline_layout, 0, &layout.set_layouts, &offsets);
        cmd.bind_pipeline(pipeline);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_set(
        gpu: &Arc<Gpu>,
        shader_stage: ShaderStage,
        slots: &SlotUsage,
        set_kind: u32,
        offsets: &[u64; 32],
        ptr: *mut u8,
        cmd: &CommandList,
    ) {
        let writer = DescriptorWriter::new(gpu, offsets, ptr);
        let resources = cmd.resources(shader_stage);

        match set_kind {
            dxil2spirv::SET_CBV => {
                for slot in SlotUsage::iter_mask(slots.cbv) {
                    writer.write_uniform_buffer(
                        slot,
                        resources.get_shader_constant_buffer_view(slot),
                    );
                }
            }
            dxil2spirv::SET_SRV => {
                for slot in SlotUsage::iter_mask(slots.srv) {
                    match slots.srv_types[slot as usize] {
                        SlotType::UniformTexelBuffer => {
                            writer.write_uniform_texel_buffer(slot, resources.get_srv_buffer(slot));
                        }
                        SlotType::StorageTexelBuffer => {
                            writer.write_storage_texel_buffer(slot, resources.get_srv_buffer(slot));
                        }
                        SlotType::StorageBuffer => {
                            let buffer = resources.get_srv_buffer(slot);
                            writer.write_storage_buffer(
                                slot,
                                buffer.map(|b| ConstantBufferView {
                                    addr: b.addr,
                                    size: b.size,
                                }),
                            );
                        }
                        _ => writer
                            .write_sampled_image(slot, resources.get_shader_resource_view(slot)),
                    }
                }
            }
            dxil2spirv::SET_SAMPLER => {
                for slot in SlotUsage::iter_mask(slots.sampler) {
                    writer.write_sampler(slot, resources.get_sampler(slot));
                }
            }
            dxil2spirv::SET_UAV => {
                for slot in SlotUsage::iter_mask(slots.uav) {
                    match slots.uav_types[slot as usize] {
                        SlotType::StorageBuffer => {
                            let buffer = resources.get_uav_buffer(slot);
                            writer.write_storage_buffer(
                                slot,
                                buffer.map(|b| ConstantBufferView {
                                    addr: b.addr,
                                    size: b.size,
                                }),
                            );
                        }
                        SlotType::StorageTexelBuffer => {
                            writer.write_storage_texel_buffer(slot, resources.get_uav_buffer(slot));
                        }
                        _ => writer.write_storage_image(
                            slot,
                            resources.get_unordered_access_view(slot),
                            vk::ImageLayout::GENERAL,
                        ),
                    }
                }
            }
            _ => {}
        }
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
    pub shader_stage: ShaderStage,
    pub shader_info: ShaderInfo,

    /// Does this stage have any textures bound outside of the technique's expressions?
    pub has_manual_textures: bool,
}

impl TechniqueStage {
    pub fn new(
        asset_manager: &Arc<AssetManager>,
        gpu: &Arc<Gpu>,
        _technique_data: &STechnique,
        stage: STechniqueStage,
        shader_stage: ShaderStage,
    ) -> anyhow::Result<Self> {
        let shader_info = gpu
            .pipeline_cache
            .write()
            .get_or_load_bytecode(stage.shader, shader_stage)
            .context("loading shader bytecode")?;

        let core = DynamicCore::new(gpu, asset_manager, stage.core, shader_stage)?;

        let has_manual_textures = SlotUsage::iter_mask(shader_info.slots.srv).any(|slot| {
            !core
                .textures
                .iter()
                .any(|(core_slot, _)| *core_slot == slot)
        });

        Ok(Self {
            core,
            stage: shader_stage.vk_shader_stage_flags(),
            shader_stage,
            shader_info,
            has_manual_textures,
        })
    }

    #[profiling::function]
    pub fn bind(
        &self,
        cmd: &mut CommandList,
        _full_rebind: bool,
        object_channels: Option<&AHashMap<u32, ObjectChannel>>,
    ) {
        if let Err(e) = self.core.prepare(cmd, object_channels) {
            error!("Failed to prepare technique stage: {}", e);
        }
    }
}
