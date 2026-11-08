use std::{
    f32,
    io::Write,
    ops::Deref,
    sync::{Arc, atomic::Ordering},
};

use anyhow::Context;
use ash::vk;
use bit_field::BitField;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{
    RenderStage, ShaderStage,
    features::{
        dynamic::RenderStageSubscription,
        statics::{
            SStaticInstanceTransform, SStaticMesh, SStaticMeshData, SStaticMeshInstances,
            SStaticSpecialMesh,
        },
    },
    geometry::AxisAlignedBBox,
};
use glam::{Mat4, Vec3, Vec4};
use itertools::Itertools;
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use tiger_parse::PackageManagerExt;
use tiger_pkg::TagHash;
use tiger_pkg::package_manager;

use crate::{
    asset::Handle,
    features::shared::ModelBuffers,
    gpu::{
        Gpu,
        alloc::staging::ImmutableStaging,
        buffer::ImmutableBuffer,
        command_list::{BufferResource, CommandList},
    },
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode, SubmitNode, SubmitNodeContainer},
    },
    tfx::technique::Technique,
    visibility::{ViewVisibility, bvh::Bvh},
};

use super::FeatureRenderer;

struct SpecialMesh {
    mesh: SStaticSpecialMesh,
    buffers: ModelBuffers,
    technique: Handle<Technique>,
}

impl Deref for SpecialMesh {
    type Target = SStaticSpecialMesh;

    fn deref(&self) -> &Self::Target {
        &self.mesh
    }
}

pub struct StaticModel {
    pub model: SStaticMesh,
    pub materials_by_group: Vec<Handle<Technique>>,
    pub hash: TagHash,
    pub subscribed_stages: RenderStageSubscription,
    buffers: Vec<ModelBuffers>,
    special_meshes: Vec<SpecialMesh>,
}

impl StaticModel {
    #[profiling::function]
    pub fn load(renderer: &Renderer, hash: TagHash) -> anyhow::Result<Self> {
        let model = package_manager().read_tag_struct::<SStaticMesh>(hash)?;
        let materials_by_group = model
            .techniques
            .iter()
            .map(|&tag| renderer.asset_manager.load(tag))
            .collect::<Vec<_>>();

        let buffers = model
            .opaque_meshes
            .buffers
            .iter()
            .map(
                |&(index_buffer, vertex0_buffer, vertex1_buffer, color_buffer)| {
                    ModelBuffers::load(
                        renderer,
                        vertex0_buffer,
                        vertex1_buffer,
                        index_buffer,
                        color_buffer,
                    )
                },
            )
            .collect::<anyhow::Result<Vec<_>>>()
            .context("loading opaque mesh buffers")?;

        let mut subscribed_stages = model
            .opaque_meshes
            .mesh_groups
            .iter()
            .fold(RenderStageSubscription::empty(), |acc, group| {
                acc | group.render_stage
            });

        let special_meshes = model
            .special_meshes
            .iter()
            .map(|mesh| {
                subscribed_stages |= mesh.render_stage;
                Ok(SpecialMesh {
                    mesh: mesh.clone(),
                    buffers: ModelBuffers::load(
                        renderer,
                        mesh.vertex0_buffer,
                        mesh.vertex1_buffer,
                        mesh.index_buffer,
                        mesh.color_buffer,
                    )
                    .expect("Failed to load special mesh buffers"),
                    technique: renderer.asset_manager.load(mesh.technique),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .context("loading special mesh buffers")?;

        Ok(Self {
            hash,
            model,
            materials_by_group,
            buffers,
            special_meshes,
            subscribed_stages,
        })
    }
}

pub struct StaticModelRenderer {
    // unk_cb1: ConstantBuffer<Vec4>,
    instance_buffer_offset: usize,

    model: StaticModel,
    visible_instance_ids: Vec<u32>,
    transforms: Vec<(SStaticInstanceTransform, AxisAlignedBBox)>,
    pub bounds: AxisAlignedBBox,
    identifier: u64,

    bvh: Bvh,
    precomputed_submit_nodes: Vec<(RenderStage, SubmitNode)>,
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct InstanceTransformBlock {
    pub transform: [Vec4; 3], // 0-4, 4-8, 8-12
    pub params0: Vec4,        // 12-16,
    pub params1: Vec4,        // 16-20
}

impl StaticModelRenderer {
    pub fn new(
        renderer: &Renderer,
        transforms: Vec<(SStaticInstanceTransform, AxisAlignedBBox)>,
        model_hash: TagHash,
        identifier: u64,
        transforms_upload: &ImmutableStaging,
    ) -> anyhow::Result<Self> {
        let model = StaticModel::load(renderer, model_hash)?;
        let transforms_tmp = transforms.iter().map(|(t, _)| t.clone()).collect_vec();
        let instance_data = Self::generate_constants(&model.model.opaque_meshes, &transforms_tmp);
        let instance_buffer_offset =
            transforms_upload.upload_slice(bytemuck::cast_slice(&instance_data));

        // Instance IDs dictate from where in the instance buffer to read the transform data. This is calculated as the ID * 0x50 (in bytes).
        // In the past, the engine would skip the 32 bytes where the quantization information was stored, but the offset must now be an exact multiple of 0x40 bytes.
        // cb0[0].x dictates where the quantization information  is stored. For now I've opted to just skip the first instance and use that slot for the quantization information.
        let visible_instance_ids = (0..transforms.len() as u32).map(|i| i + 1).collect_vec();

        let bounds = transforms.iter().map(|(_, b)| *b).collect_vec();
        let group_bounds = bounds.iter().cloned().sum();
        let bvh = Bvh::build(&bounds);

        let mut precomputed_submit_nodes = vec![];

        let opaque_meshes = &model.model.opaque_meshes;
        for (group_index, group, _part) in opaque_meshes
            .mesh_groups
            .iter()
            .enumerate()
            .map(|(i, g)| (i, g, &opaque_meshes.parts[g.part_index as usize]))
            .filter(|(_, _, p)| p.lod_category.is_highest_detail())
        {
            if let Some(technique) = model.materials_by_group.get(group_index) {
                let key = StaticSubmitKey {
                    technique: technique.hash().0,
                    model_index: 0, // fixed up in the StaticInstancesRenderer
                    is_special_mesh: false,
                    mesh_index: group_index as u16,
                };
                precomputed_submit_nodes.push((
                    group.render_stage,
                    SubmitNode {
                        key: key.to_u64(),
                        view_node: 0, // fixed up at submit time
                    },
                ));
            }
        }

        let special_meshes = &model.special_meshes;
        for (mesh_index, mesh) in special_meshes
            .iter()
            .enumerate()
            .filter(|(_, m)| m.lod.is_highest_detail())
        {
            let key = StaticSubmitKey {
                technique: mesh.technique.hash().0,
                model_index: 0, // fixed up in the StaticInstancesRenderer
                is_special_mesh: true,
                mesh_index: mesh_index as u16,
            };
            precomputed_submit_nodes.push((
                mesh.render_stage,
                SubmitNode {
                    key: key.to_u64(),
                    view_node: 0, // fixed up at submit time
                },
            ));
        }

        trace!(instances = transforms.len(), model_hash=%model_hash, "Loading model");
        Ok(Self {
            // unk_cb1: ConstantBuffer::create(gpu, Some(&Vec4::ZERO))?, // Offsets instance buffer data
            instance_buffer_offset,
            model,
            bounds: group_bounds,
            bvh,
            visible_instance_ids,
            transforms,
            identifier,
            precomputed_submit_nodes,
        })
    }

    #[profiling::function]
    pub fn render_group(&self, cmd: &mut CommandList, is_special_mesh: bool, mesh_index: usize) {
        cmd.enable_smart_technique_binding();

        // let is_opaque = matches!(
        //     stage,
        //     RenderStage::ShadowGenerate | RenderStage::DepthPrepass | RenderStage::GenerateGbuffer
        // );

        cmd.set_shader_resource_view(ShaderStage::Vertex, 0, None);
        if is_special_mesh {
            let mesh = &self.model.special_meshes[mesh_index];
            if mesh.buffers.bind(cmd).is_none() {
                return;
            }

            cmd.set_input_layout(mesh.input_layout_index as usize);
            cmd.set_input_topology(mesh.primitive_type);
            if mesh
                .technique
                .get_ref(|technique| technique.bind(cmd))
                .is_none()
            {
                return;
            }

            cmd.draw_indexed(
                mesh.index_range(),
                0..self.visible_instance_ids.len() as u32,
                0,
            );
        } else {
            let opaque_meshes = &self.model.model.opaque_meshes;
            let group = &opaque_meshes.mesh_groups[mesh_index];
            let part = &opaque_meshes.parts[group.part_index as usize];
            let buffers = &self.model.buffers[part.buffer_index as usize];
            if buffers.bind(cmd).is_none() {
                return;
            }

            cmd.set_input_layout(group.input_layout_index as usize);
            cmd.set_input_topology(part.primitive_type);

            if self
                .model
                .materials_by_group
                .get(mesh_index)
                .and_then(|handle| handle.get_ref(|technique| technique.bind(cmd)))
                .is_none()
            {
                return;
            }

            cmd.draw_indexed(
                part.index_range(),
                0..self.visible_instance_ids.len() as u32,
                0,
            );
        }
    }

    #[profiling::function]
    fn generate_constants(
        model: &SStaticMeshData,
        transforms: &[SStaticInstanceTransform],
        // ao: Option<&SStaticAmbientOcclusion>,
    ) -> Vec<u8> {
        let mut buffer = vec![];

        buffer
            .write_all(bytemuck::cast_slice(&[
                model.mesh_offset.x,
                model.mesh_offset.y,
                model.mesh_offset.z,
                model.mesh_scale,
                model.texture_coordinate_scale,
                model.texture_coordinate_offset.x,
                model.texture_coordinate_offset.y,
                f32::from_bits(model.max_color_index),
            ]))
            .unwrap();

        buffer
            .write_all(&(transforms.len() as f32).to_le_bytes())
            .unwrap();

        while buffer.len() < size_of::<InstanceTransformBlock>() {
            buffer.write_all(&[0u8]).unwrap();
        }

        // let model_transform = Mat4::from_cols_array_2d(&[
        //     [model.mesh_scale, 0.0, 0.0, model.mesh_offset.x],
        //     [0.0, model.mesh_scale, 0.0, model.mesh_offset.y],
        //     [0.0, 0.0, model.mesh_scale, model.mesh_offset.z],
        //     [0.0, 0.0, 0.0, 1.0],
        // ]);
        for transform in transforms {
            let instance_transform = Mat4::from_scale_rotation_translation(
                Vec3::splat(transform.scale),
                transform.rotation,
                transform.translation,
            )
            .transpose();
            // let instance_transform = Mat4::IDENTITY;

            // let matrix = instance_transform;
            // let vertex_ao_offset = if let Some(vao_base) = vao_base {
            //     transform.vertex_ao_offset + vao_base
            // } else {
            //     transform.vertex_ao_offset
            // };

            buffer
                .write_all(bytemuck::cast_slice(&[InstanceTransformBlock {
                    transform: [
                        instance_transform.x_axis,
                        instance_transform.y_axis,
                        instance_transform.z_axis,
                    ],
                    params0: Vec4::new(
                        1.0,
                        1.0,
                        1.0,
                        f32::from_bits(0),
                        // Quantization block offset
                        // f32::from_bits(
                        //     ao_offsets
                        //         .get(i)
                        //         .copied()
                        //         .map(|v| v.shr(2))
                        //         .unwrap_or(0x02000000),
                        // ),
                    ),
                    params1: Vec4::ZERO,
                }]))
                .unwrap();
        }

        buffer
    }
}

pub struct StaticInstancesRenderer {
    subscribed_stages: RenderStageSubscription,
    /// (model, transforms buffer SRV)
    models: Vec<(StaticModelRenderer, BufferResource)>,
    _transforms_buffer: ImmutableBuffer,
}

impl StaticInstancesRenderer {
    pub fn load_from_tag(renderer: &Renderer, instances_hash: TagHash) -> anyhow::Result<Self> {
        let instances: SStaticMeshInstances = package_manager().read_tag_struct(instances_hash)?;
        let transforms_upload = ImmutableStaging::new(instances.instance_groups.len() * 0x50 * 2);
        let mut models = instances
            .instance_groups
            .par_iter()
            .map(|group| {
                let model = instances.statics[group.static_index as usize];
                let range = (group.instance_start as usize)
                    ..(group.instance_start + group.instance_count) as usize;

                let renderer = StaticModelRenderer::new(
                    renderer,
                    instances.transforms[range.clone()]
                        .iter()
                        .cloned()
                        .zip(
                            instances.occlusion_bounds.bounds[range]
                                .iter()
                                .map(|b| &b.bb)
                                .cloned(),
                        )
                        .collect(),
                    model,
                    instances.vertex_ao_identifier,
                    &transforms_upload,
                )?;

                Ok(renderer)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        models
            .iter_mut()
            .enumerate()
            .for_each(|(model_index, model)| {
                for (_, node) in &mut model.precomputed_submit_nodes {
                    let mut key = StaticSubmitKey::from_u64(node.key);
                    key.model_index = model_index as u16;
                    node.key = key.to_u64();
                }
            });

        Ok(Self::new(&renderer.gpu, models, transforms_upload))
    }

    pub fn new(
        gpu: &Arc<Gpu>,
        models: Vec<StaticModelRenderer>,
        transforms_upload: ImmutableStaging,
    ) -> Self {
        let transforms_data = transforms_upload.into_inner();
        let transforms_buffer = ImmutableBuffer::new(
            gpu,
            "grouped_transforms_buffer",
            vk::Format::R32_UINT,
            &transforms_data,
        )
        .unwrap();

        Self {
            subscribed_stages: models
                .iter()
                .fold(RenderStageSubscription::empty(), |acc, m| {
                    acc | m.model.subscribed_stages
                }),
            models: models
                .into_iter()
                .map(|m| {
                    let start = m.instance_buffer_offset as u64;
                    let count = (m.transforms.len() + 1) * size_of::<InstanceTransformBlock>();
                    let transforms_srv = transforms_buffer.srv(start, count as u64);
                    (m, transforms_srv)
                })
                .collect(),
            _transforms_buffer: transforms_buffer,
        }
    }
}

impl FeatureRenderer for StaticInstancesRenderer {
    fn visibility_test(&mut self, visibility: &ViewVisibility) {
        self.models.par_iter_mut().for_each(|(model, _)| {
            model.visible_instance_ids.clear();
            model
                .bvh
                .collect_visible_leaves(visibility, &mut model.visible_instance_ids, 1);
            assert!(
                model.visible_instance_ids.len() <= model.transforms.len(),
                "Visible instance count exceeds transform count"
            );
        });
    }

    #[profiling::function]
    fn populate_submit_node_blocks(
        &self,
        renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        _frame_node: &RenderPerFrameNode,
        visibility: &ViewVisibility,
        submit_node_blocks: &mut SubmitNodeContainer,
    ) {
        for (model, _) in self
            .models
            .iter()
            .filter(|(m, _)| !m.visible_instance_ids.is_empty())
        {
            renderer
                .gpu
                .num_static_instances
                .fetch_add(model.transforms.len(), Ordering::Relaxed);
            for &(stage, mut node) in model
                .precomputed_submit_nodes
                .iter()
                .filter(|(stage, _)| visibility.render_stages.is_subscribed(*stage))
            {
                node.view_node = view_node;
                submit_node_blocks.push(stage, node);
            }
        }
    }

    fn submit(
        &self,
        cmd: &mut CommandList,
        _stage: RenderStage,
        _frame_node: &RenderPerFrameNode,
        _view_node: &RenderPerViewNode,
        submit_key: u64,
    ) {
        let key = StaticSubmitKey::from_u64(submit_key);

        let (model, transforms_srv) = &self.models[key.model_index as usize];
        let ids_slice = match cmd
            .gpu()
            .frame()
            .upload
            .upload_bytes(bytemuck::cast_slice(&model.visible_instance_ids))
        {
            Ok(va) => va,
            Err(e) => {
                error!("Failed to upload instance IDs: {}", e);
                return;
            }
        };
        cmd.set_vertex_buffers(2, &[ids_slice.buffer()], &[ids_slice.offset() as u64]);
        cmd.set_shader_resource_buffer(ShaderStage::Vertex, 2, Some(*transforms_srv));
        model.render_group(cmd, key.is_special_mesh, key.mesh_index as usize);
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        self.subscribed_stages
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct StaticSubmitKey {
    pub technique: u32,
    pub model_index: u16,
    pub is_special_mesh: bool,
    pub mesh_index: u16,
}

impl StaticSubmitKey {
    pub fn from_u64(k: u64) -> Self {
        Self {
            technique: k.get_bits(32..64) as u32,
            model_index: k.get_bits(16..32) as u16,
            is_special_mesh: k.get_bit(15),
            mesh_index: k.get_bits(0..15) as u16,
        }
    }

    pub fn to_u64(self) -> u64 {
        let mut k = 0u64;

        k.set_bits(0..15, self.mesh_index as u64);
        k.set_bit(15, self.is_special_mesh);
        k.set_bits(16..32, self.model_index as u64);
        k.set_bits(32..64, self.technique as u64);

        k
    }
}
