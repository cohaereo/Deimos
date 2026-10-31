use std::sync::Arc;

use ahash::AHashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::{
    tag::WideHash,
    tfx::{ExternIndex, SDynamicCore, SSamplerReference, ShaderStage},
};
use deimos_ecs::object::ObjectChannel;
use glam::Vec4;
use itertools::Itertools;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    asset::{AssetManager, Handle, texture::Texture},
    gpu::{
        Gpu,
        command_list::{CommandList, ConstantBufferView},
    },
    tfx::{
        d3d12_sampler::D3D12SamplerDesc,
        expression_vm::{
            self,
            interpreter::InterpreterState,
            opcodes::{
                ExpressionDataSource, Opcode, OpcodeIterator, get_data_access_from_bytecode,
            },
        },
        externs::{ExternAccessorExt, TextureView, Uav},
    },
};

pub enum ResolvedTextureSource {
    Static(Handle<Texture>),
    Dynamic {
        extern_index: ExternIndex,
        offset: u32,
    },
}

pub enum UavSource {
    Dynamic {
        extern_index: ExternIndex,
        offset: u32,
    },
}

/// Shared core for dynamic textures/samplers/constants used by scopes and techniques
///
/// # D3D12 Related Notes
/// - Samplers are extracted and filtered from bytecode, and used as static samplers in the pipeline state object.
/// - Dynamic textures are also extracted, and joined together with static textures in the `textures` array.
pub struct DynamicCore {
    pub stage: ShaderStage,

    data: SDynamicCore,

    pub samplers: Vec<(u32, vk::Sampler)>,
    pub textures: Vec<(u32, ResolvedTextureSource)>,
    pub uavs: Vec<(u32, UavSource)>,
    has_dynamic_textures: bool,

    pub initial_constants: Vec<Vec4>,
    pub cbuffer_size: usize,

    pub data_sources: ExpressionDataSource,
}

impl DynamicCore {
    pub fn new(
        gpu: &Gpu,
        asset_manager: &Arc<AssetManager>,
        data: SDynamicCore,
        stage: ShaderStage,
    ) -> anyhow::Result<Self> {
        let mut core = Self {
            stage,
            data,
            samplers: Vec::new(),
            textures: Vec::new(),
            uavs: Vec::new(),
            initial_constants: Vec::new(),
            cbuffer_size: 0,
            data_sources: ExpressionDataSource::empty(),
            has_dynamic_textures: false,
        };

        let mut resources = DynamicCoreResources::extract(gpu, &core.data, stage)?;
        std::mem::swap(&mut resources.samplers, &mut core.samplers);
        std::mem::swap(&mut resources.uavs, &mut core.uavs);

        for (slot, t) in resources.textures {
            match t {
                TextureSource::Static(tag) => {
                    let texture = asset_manager.load(tag);
                    core.textures
                        .push((slot, ResolvedTextureSource::Static(texture)));
                }
                TextureSource::Dynamic {
                    extern_index,
                    offset,
                } => {
                    core.has_dynamic_textures = true;
                    core.textures.push((
                        slot,
                        ResolvedTextureSource::Dynamic {
                            extern_index,
                            offset,
                        },
                    ));
                }
            }
        }

        core.data.bytecode =
            filter_bytecode_assignments(&core.data.bytecode).context("filtering bytecode")?;

        core.data_sources = get_data_access_from_bytecode(&core.data.bytecode)
            .context("getting data access from bytecode")?;

        core.initial_constants = if core.data.constant_buffer.is_some() {
            let entry = package_manager()
                .get_entry(core.data.constant_buffer)
                .context("Failed to get cbuffer tag entry")?;

            let data = package_manager().read_tag(entry.reference)?;
            let vec4s = bytemuck::cast_slice(&data);
            vec4s.to_vec()
        } else {
            let vec4s = &core.data.unk30;
            if vec4s.is_empty() {
                vec![]
            } else {
                vec4s.clone()
            }
        };
        core.cbuffer_size = core.initial_constants.len() * size_of::<Vec4>();

        Ok(core)
    }

    pub const fn cbuffer_slot(&self) -> i32 {
        self.data.constant_buffer_slot
    }

    #[profiling::function]
    pub fn prepare(
        &self,
        cmd: &mut CommandList,
        object_channels: Option<&AHashMap<u32, ObjectChannel>>,
    ) -> anyhow::Result<()> {
        if self.data.constant_buffer_slot >= 0 {
            let mut buffer = cmd
                .gpu()
                .frame()
                .upload
                .alloc_slice(self.cbuffer_size)
                .context("allocating cbuffer")?;

            let out = bytemuck::cast_slice_mut(buffer.as_mut_slice());
            out.copy_from_slice(&self.initial_constants);
            let mut interpreter =
                InterpreterState::new(&self.data.bytecode).with_externs(&cmd.externs);
            if let Some(object_channels) = object_channels {
                interpreter = interpreter.with_object_channels(object_channels);
            }

            if let Err(e) = interpreter.evaluate(&self.data.bytecode_constants, out) {
                error!("Failed to evaluate expression bytecode: {:?}", e);

                let bytecode_listing = match expression_vm::disassemble(&self.data.bytecode) {
                    Ok(ops) => ops.into_iter().map(|v| format!("    {v}")).join("\n"),
                    Err(e) => {
                        format!("Failed to disassemble bytecode: {e:?}")
                    }
                };
                debug!("Bytecode:\n{}", bytecode_listing);

                if interpreter.ip < self.data.bytecode.len() {
                    // Patch the bytecode to disable the expression
                    unsafe {
                        self.data
                            .bytecode
                            .as_ptr()
                            .add(interpreter.ip)
                            .cast_mut()
                            .write(expression_vm::opcodes::Opcode::ExtReturn as u8);
                    }
                }
            }

            cmd.set_shader_constant_buffer_view(
                self.stage,
                self.data.constant_buffer_slot as u32,
                Some(ConstantBufferView {
                    addr: buffer.virtual_address(),
                    size: self.cbuffer_size as u64,
                }),
            );
        }

        for (slot, sampler) in self.samplers.iter() {
            cmd.set_shader_sampler(self.stage, *slot, Some(*sampler));
        }

        for (slot, uav) in self.uavs.iter() {
            match uav {
                UavSource::Dynamic {
                    extern_index,
                    offset,
                } => {
                    if let Some(view) = cmd
                        .externs
                        .get_extern_value::<Uav>(*extern_index, *offset as usize)
                        .and_then(|u| u.get_uav())
                    {
                        cmd.set_unordered_access_view(self.stage, *slot, Some(view));
                    }
                }
            }
        }

        for (slot, texture) in self.textures.iter() {
            match texture {
                ResolvedTextureSource::Static(handle) => {
                    handle
                        .get_ref(|v| cmd.set_shader_resource_view(self.stage, *slot, Some(v.view)));
                }
                ResolvedTextureSource::Dynamic {
                    extern_index,
                    offset,
                } => {
                    if let Some(view) = cmd
                        .externs
                        .get_extern_value::<TextureView>(*extern_index, *offset as usize)
                        .and_then(|v| v.get_srv())
                    {
                        cmd.set_shader_resource_view(self.stage, *slot, Some(view));
                    } else {
                        match cmd.gpu().magic_textures.get_texture(
                            cmd.gpu(),
                            format!("{extern_index:?}->_0x{offset:X}"),
                            extern_filler(*extern_index, *offset),
                        ) {
                            Ok(srv) => {
                                cmd.set_shader_resource_view(self.stage, *slot, Some(srv));
                            }
                            Err(e) => {
                                error!(
                                    "Failed to create magic texture for {extern_index:?}+0x{offset:X}: {e:?}"
                                );
                                cmd.set_shader_resource_view(self.stage, *slot, None);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn all_textures_loaded(&self) -> bool {
        self.textures.iter().all(|(_, tex)| {
            if let ResolvedTextureSource::Static(handle) = tex {
                handle.is_loaded()
            } else {
                true
            }
        })
    }

    /// Does this dynamic core resolve dynamic textures?
    ///
    /// **Note: Techniques may have manually bound textures that aren't referenced by the dynamic core (eg. terrain dyemap)**
    pub const fn has_dynamic_textures(&self) -> bool {
        self.has_dynamic_textures
    }
}

const fn extern_filler(index: ExternIndex, offset: u32) -> Option<[u8; 4]> {
    match (index, offset) {
        (ExternIndex::ShadowMask, _) => Some([255, 255, 255, 255]),
        (ExternIndex::Atmosphere, 0xF0) => Some([0x00, 0x00, 0x00, 0xFF]),
        (ExternIndex::Atmosphere, _) => Some([0x00, 0x00, 0x00, 0xFF]),
        (ExternIndex::Transparent, 0) => Some([0x00, 0x00, 0x00, 0x00]),
        (ExternIndex::Transparent, 0x50) => Some([0xFF, 0xFF, 0xFF, 0xFF]),
        (ExternIndex::Transparent, _) => Some([0x00, 0x00, 0x00, 0xFF]),
        _ => None,
    }
}

pub enum TextureSource {
    Static(WideHash),
    Dynamic {
        extern_index: ExternIndex,
        offset: u32,
    },
}

/// Resources extracted from dynamic core
pub struct DynamicCoreResources {
    pub samplers: Vec<(u32, vk::Sampler)>,
    pub textures: Vec<(u32, TextureSource)>,
    pub uavs: Vec<(u32, UavSource)>,
}

impl DynamicCoreResources {
    pub fn extract(gpu: &Gpu, core: &SDynamicCore, stage: ShaderStage) -> anyhow::Result<Self> {
        let mut res = Self {
            samplers: Vec::new(),
            textures: Vec::new(),
            uavs: Vec::new(),
        };

        res.textures.extend(
            core.textures
                .iter()
                .map(|t| (t.slot, TextureSource::Static(t.texture))),
        );

        let mut sampler_tags = Vec::new();

        extract_textures_and_samplers(
            &core.bytecode,
            &core.samplers,
            &mut sampler_tags,
            &mut res.textures,
            &mut res.uavs,
        )?;

        let mut sampler_cache = gpu.sampler_cache.lock();
        for (slot, tag) in sampler_tags {
            let sampler_entry = package_manager()
                .get_entry(tag)
                .context("missing entry for sampler")?;
            let data = package_manager()
                .read_tag(sampler_entry.reference)
                .context("reading sampler")?;
            let desc: D3D12SamplerDesc = unsafe { data.as_ptr().cast::<D3D12SamplerDesc>().read() };
            let sampler = sampler_cache
                .entry(desc.clone())
                .or_insert_with(|| unsafe { gpu.create_sampler(&desc.to_vk(), None).unwrap() });

            res.samplers.push((slot, *sampler));
        }

        Ok(res)
    }
}

fn extract_textures_and_samplers(
    bytecode: &[u8],
    static_samplers: &[SSamplerReference],
    samplers: &mut Vec<(u32, TagHash)>,
    textures: &mut Vec<(u32, TextureSource)>,
    uavs: &mut Vec<(u32, UavSource)>,
) -> anyhow::Result<()> {
    let mut last_sampler = None;
    let mut last_texture = None;
    let mut last_uav = None;

    let bytecode = OpcodeIterator::new(bytecode);
    for op in bytecode {
        let (op, ptr) = op?;

        match op {
            Opcode::PushSamplerState => {
                let index = ptr[0];
                let sampler = static_samplers
                    .get(index as usize)
                    .context("Invalid sampler index")?;
                last_sampler = Some(sampler.sampler);
            }
            Opcode::PopSamplerState => {
                let slot = ptr[0] & 0x1F;
                let sampler = last_sampler
                    .take()
                    .context("Sampler state was not pushed")?;
                samplers.push((slot as u32, sampler));
            }
            Opcode::PushExternInputTextureView => {
                let extern_id = ExternIndex::try_from(ptr[0])
                    .ok()
                    .context("Invalid extern index")?;
                let offset = ptr[1] as u32 * 8;
                last_texture = Some(TextureSource::Dynamic {
                    extern_index: extern_id,
                    offset,
                });
            }
            Opcode::PopTextureView => {
                let slot = ptr[0] & 0x1F;
                let texture = last_texture.take().context("Texture view was not pushed")?;
                textures.push((slot as u32, texture));
            }
            Opcode::PushExternInputUav => {
                let extern_id = ExternIndex::try_from(ptr[0])
                    .ok()
                    .context("Invalid extern index")?;
                let offset = ptr[1] as u32 * 8;
                last_uav = Some(UavSource::Dynamic {
                    extern_index: extern_id,
                    offset,
                });
            }
            Opcode::PopUav => {
                let slot = ptr[0] & 0x1F;
                let uav = last_uav.take().context("UAV was not pushed")?;
                uavs.push((slot as u32, uav));
            }
            _ => {
                anyhow::ensure!(
                    last_sampler.is_none() && last_texture.is_none() && last_uav.is_none(),
                    "Malformed bytecode. Sampler/texture/uav was acquired but not assigned"
                );
            }
        }
    }

    Ok(())
}

fn filter_bytecode_assignments(original: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut new = Vec::with_capacity(original.len());

    let mut offset = 0;
    let bytecode = OpcodeIterator::new(original);
    for op in bytecode {
        let (op, _) = op?;

        if matches!(
            op,
            Opcode::PushSamplerState
                | Opcode::PopSamplerState
                | Opcode::PushExternInputTextureView
                | Opcode::PopTextureView
                | Opcode::PushExternInputUav
                | Opcode::PopUav
        ) {
            offset += op.size();
            continue;
        }

        new.extend_from_slice(&original[offset..offset + op.size()]);

        offset += op.size();
    }

    Ok(new)
}
