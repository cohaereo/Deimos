use std::{ops::Range, sync::atomic::Ordering};

use ash::vk;
use bytemuck::cast_slice;
use deimos_data::tfx::{
    ShaderStage,
    features::{decorators::SDecorator, dynamic::RenderStageSubscription},
    geometry::AxisAlignedBBox,
};
use glam::{Mat4, Vec4};
use parking_lot::RwLock;
use tiger_pkg::TagHash;

use crate::{
    asset::vertex_buffer::VertexBuffer,
    features::{
        DETAIL_LOD_DISTANCE_RATIOS, FeatureRenderer, LOD_ENABLED, dynamic_model::DynamicModel,
        lod_for_sphere_with_ratios,
    },
    gpu::buffer::ImmutableBuffer,
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode},
    },
    tfx::externs,
    visibility::{ViewVisibility, bvh::Bvh},
};

/// Cull clusters beyond this distance
const DECORATOR_CULL_DISTANCE_RATIO: f32 = 32.0;

#[derive(Clone, Copy)]
struct DecoratorDraw {
    group_index: u16,
    quality: u8,
    offset: usize,
    instance_count: u32,
}

#[derive(Default)]
struct DecoratorViewData {
    visible: Vec<u32>,
    segments: Vec<(u16, u8, Range<u32>)>,
    compacted: Vec<u8>,
    draws: Vec<DecoratorDraw>,
}

pub struct DecoratorRenderer {
    pub data: SDecorator,
    pub hash: TagHash,
    #[allow(clippy::type_complexity)]
    pub models: Vec<(
        Box<DynamicModel>,
        Box<externs::RigidModel>,
        Option<ImmutableBuffer>,
    )>,
    group_quality_ranges: Vec<Range<u8>>,
    instance_raw: Vec<u8>,
    instance_stride: usize,
    cluster_bounds: Vec<AxisAlignedBBox>,
    cluster_bvh: Option<Bvh>,
    cluster_segments: Vec<Vec<(u16, Range<u32>)>>,
    view_data: DecoratorViewData,
    uploaded: RwLock<Option<(vk::Buffer, usize)>>,
    instance_blend_indices_vb: VertexBuffer,
}

impl DecoratorRenderer {
    pub fn load(renderer: &Renderer, hash: TagHash, decorator: SDecorator) -> anyhow::Result<Self> {
        let mut models = vec![];
        let mut model_quality_ranges = vec![];
        for smodel in &decorator.unk8 {
            let model = DynamicModel::load(renderer, smodel.entity_model, vec![], vec![])?;
            let ext = Box::new(externs::RigidModel {
                local_to_world: Mat4::IDENTITY,
                position_scale: model.model.model_scale,
                position_offset: model.model.model_offset,
                texcoord0_scale_offset: Vec4::new(
                    model.model.texcoord_scale.x,
                    model.model.texcoord_scale.y,
                    model.model.texcoord_offset.x,
                    model.model.texcoord_offset.y,
                ),
                dynamic_sh_ao_values: Vec4::new(1.0, 1.0, 1.0, 0.0),
                ..Default::default()
            });

            let speedtree_cbuffer = if let Some(unk34) = &*smodel.unk14 {
                let mut data = vec![Vec4::ONE; 72];
                data[0..=4].copy_from_slice(&unk34.unk8.get(1).unwrap_or(&unk34.unk8[0]).unk0);
                // data[5..=29]
                for (i, c) in smodel.unk38.as_chunks::<4>().0.iter().enumerate() {
                    let v = Vec4::new(c[0], c[1], c[2], c[3]);
                    data[5 + i] = v;
                }
                // data[30..=33]
                data[30] = Vec4::X;
                data[31] = Vec4::Y;
                data[32] = Vec4::Z;
                data[33] = Vec4::W;
                let ndata: [u8; 0x140] = [
                    0x8B, 0xF0, 0xB6, 0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xBB, 0x0D, 0x02, 0x43, 0x9E, 0x09, 0xE9, 0x3D, 0x70, 0x73,
                    0x2B, 0x43, 0x36, 0xC7, 0x2E, 0x3E, 0x9E, 0x45, 0x54, 0x3A, 0x83, 0xAB, 0x04,
                    0x3C, 0x83, 0xAB, 0x04, 0x3C, 0x00, 0x00, 0x00, 0x00, 0xD0, 0x21, 0x3D, 0x3D,
                    0xEE, 0x16, 0x05, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x90, 0x3F, 0x67, 0x66, 0xE6, 0x3E, 0x00, 0x00, 0x90, 0x3F, 0x67, 0x66,
                    0xE6, 0x3E, 0x47, 0xB6, 0xDD, 0x3E, 0x47, 0xB6, 0xDD, 0x3E, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x23, 0xB1, 0x42, 0xCD,
                    0x67, 0x0D, 0x3E, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0xC0, 0xF1,
                    0x7D, 0x42, 0x92, 0xDB, 0x8B, 0x3C, 0xA5, 0x97, 0x4F, 0x3E, 0x00, 0x00, 0xA0,
                    0x3F, 0xD2, 0x34, 0x06, 0x44, 0xBE, 0xDE, 0xB3, 0x3C, 0xCD, 0xCC, 0x4C, 0x3F,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x33, 0x33, 0x33, 0x3F, 0x00,
                    0x00, 0x40, 0x40, 0x9A, 0x99, 0x99, 0x3E, 0x31, 0x4C, 0x9F, 0x42, 0x0D, 0x1A,
                    0x7A, 0x3C, 0x0B, 0x4E, 0x02, 0x3C, 0x00, 0x00, 0x00, 0x00, 0xCA, 0xB0, 0xA2,
                    0x3C, 0x40, 0x87, 0x87, 0x3E, 0xF4, 0xD6, 0x60, 0xC3, 0x00, 0x00, 0x00, 0x00,
                    0x09, 0x0A, 0x8F, 0x44, 0x94, 0xC1, 0x30, 0x3E, 0x00, 0x00, 0x00, 0x3F, 0x00,
                    0x00, 0x00, 0x00, 0x9A, 0x99, 0x99, 0x3F, 0xCD, 0xCC, 0x4C, 0x3F, 0x9A, 0x99,
                    0x99, 0x3E, 0x18, 0xB7, 0x51, 0x38, 0x76, 0x99, 0xB3, 0xC2, 0x82, 0xCC, 0x76,
                    0x3D, 0x72, 0xCC, 0xF6, 0x3D, 0x99, 0xAB, 0x84, 0x3B, 0x72, 0xCC, 0xF6, 0x3D,
                    0x15, 0x85, 0x2B, 0x45, 0x1D, 0x5B, 0x1F, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0xC0, 0x3F, 0x00, 0x00, 0xC0, 0x3F, 0x67, 0x66, 0xE6, 0x3E, 0x00, 0x00,
                    0x00, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x2E, 0xBD, 0xBB, 0xB3, 0x00, 0x00, 0x80, 0xBF,
                    0x00, 0x00, 0x00, 0x00, 0xBA, 0x26, 0x09, 0x3E,
                ];
                // data[34..=53]
                // copy raw data
                let data_vec4: &[[f32; 4]] = bytemuck::cast_slice(&ndata);
                for (i, v) in data_vec4.iter().enumerate() {
                    data[34 + i] = Vec4::from_array(*v);
                }
                Some(ImmutableBuffer::new(
                    &renderer.gpu,
                    "speedtree_cbuffer",
                    vk::Format::R32_SFLOAT,
                    cast_slice(&data),
                )?)
            } else {
                None
            };

            let (min_quality, max_quality) = model
                .model
                .meshes
                .iter()
                .flat_map(|mesh| mesh.parts.iter().map(|part| part.unk17))
                .fold((u8::MAX, 0), |(min, max), quality| {
                    (min.min(quality), max.max(quality))
                });
            let min_quality = if min_quality == u8::MAX {
                0
            } else {
                min_quality
            };
            model_quality_ranges.push(min_quality..max_quality);

            models.push((model, ext, speedtree_cbuffer));
        }

        if models.is_empty() {
            anyhow::bail!("No models found in decorator");
        }

        if models.len() > 1 {
            // anyhow::bail!("Decorators with more than one model are not supported yet");
            warn!("Decorators with more than one model are WIP");
        }

        let instance_count = decorator.unk48.instance_data.data.len();
        let (instance_raw, instance_stride) =
            VertexBuffer::get_raw_data_and_stride(decorator.unk48.instance_buffer)?;
        let instance_stride = instance_stride as usize;
        if instance_stride == 0 {
            anyhow::bail!("Decorator {hash} has a zero instance stride");
        }
        if instance_raw.len() != instance_count * instance_stride {
            warn!(
                decorator_set = %hash,
                instances = instance_count,
                raw_instances = instance_raw.len() / instance_stride,
                "Decorator instance count does not match the instance buffer"
            );
        }
        if decorator.unk38.last().copied() != Some(instance_count as u32) {
            warn!(
                decorator_set = %hash,
                instances = instance_count,
                cluster_end = ?decorator.unk38.last(),
                "Decorator cluster ranges do not cover all instances"
            );
        }
        if decorator.unk18.last().copied() != Some(instance_count as u32) {
            warn!(
                decorator_set = %hash,
                instances = instance_count,
                group_end = ?decorator.unk18.last(),
                "Decorator group ranges do not cover all instances"
            );
        }

        let group_bounds = &decorator.unk18;
        let group_count = group_bounds.len().saturating_sub(1);
        let group_quality_ranges = (0..group_count)
            .map(|group| {
                let model_id = if models.len() == 1 { 0 } else { group };
                model_quality_ranges.get(model_id).cloned().unwrap_or(0..3)
            })
            .collect::<Vec<_>>();

        let cluster_bounds = decorator
            .unk4c
            .bounds
            .iter()
            .map(|bound| bound.bb)
            .collect::<Vec<_>>();
        let cluster_bvh = if cluster_bounds.is_empty() {
            None
        } else {
            Some(Bvh::build(&cluster_bounds))
        };

        let cluster_ranges = &decorator.unk38;
        let mut cluster_segments = Vec::with_capacity(cluster_bounds.len());
        if cluster_ranges.len() == cluster_bounds.len() + 1 && group_bounds.len() >= 2 {
            let mut group = 0usize;
            for cluster in 0..cluster_bounds.len() {
                let start = cluster_ranges[cluster];
                let end = cluster_ranges[cluster + 1];

                while group + 1 < group_bounds.len() && group_bounds[group + 1] <= start {
                    group += 1;
                }

                let mut segments = Vec::new();
                let mut current = group;
                while current + 1 < group_bounds.len() && group_bounds[current] < end {
                    let segment_start = start.max(group_bounds[current]);
                    let segment_end = end.min(group_bounds[current + 1]);
                    if segment_start < segment_end {
                        segments.push((current as u16, segment_start..segment_end));
                    }
                    current += 1;
                }

                cluster_segments.push(segments);
            }
        } else {
            warn!(
                decorator_set = %hash,
                clusters = cluster_bounds.len(),
                cluster_ranges = cluster_ranges.len(),
                groups = group_bounds.len(),
                "Decorator occlusion bounds do not match the instance ranges"
            );
            cluster_segments.resize_with(cluster_bounds.len(), Vec::new);
        }

        // u8 for decorators, f32 for speedtree
        // let blend_index_data = vec![0xC8u8; decorator.unk48.instance_data.data.len() * 4];
        let blend_index_data = vec![1f32; instance_count * 4];
        let instance_blend_indices_vb =
            VertexBuffer::load_data(&renderer.gpu, bytemuck::cast_slice(&blend_index_data), 4)?;

        Ok(Self {
            models,
            group_quality_ranges,
            instance_raw,
            instance_stride,
            cluster_bounds,
            cluster_bvh,
            cluster_segments,
            view_data: DecoratorViewData::default(),
            uploaded: RwLock::new(None),
            hash,
            instance_blend_indices_vb,
            data: decorator,
        })
    }
}

impl FeatureRenderer for DecoratorRenderer {
    fn visibility_test(&mut self, visibility: &ViewVisibility) {
        let Some(bvh) = &self.cluster_bvh else {
            self.view_data = DecoratorViewData::default();
            return;
        };

        let DecoratorViewData {
            visible,
            segments,
            compacted,
            draws,
        } = &mut self.view_data;
        visible.clear();
        segments.clear();
        compacted.clear();
        draws.clear();

        bvh.collect_visible_leaves(visibility, visible, 0);

        let lod_enabled = LOD_ENABLED.load(Ordering::Relaxed);

        for &cluster in visible.iter() {
            let Some(bounds) = self.cluster_bounds.get(cluster as usize) else {
                continue;
            };
            let center = bounds.centroid();
            let radius = bounds.radius();
            let distance = center.distance(visibility.position);
            if distance > radius.max(1.0) * DECORATOR_CULL_DISTANCE_RATIO {
                continue;
            }

            let quality = if lod_enabled {
                lod_for_sphere_with_ratios(
                    center,
                    radius,
                    visibility.position,
                    &DETAIL_LOD_DISTANCE_RATIOS,
                    3,
                )
            } else {
                1 // medium
            };

            let Some(cluster_segments) = self.cluster_segments.get(cluster as usize) else {
                continue;
            };
            for &(group, ref segment_range) in cluster_segments {
                let quality_range = self
                    .group_quality_ranges
                    .get(group as usize)
                    .cloned()
                    .unwrap_or(0..3);
                segments.push((
                    group,
                    quality.clamp(quality_range.start, quality_range.end),
                    segment_range.clone(),
                ));
            }
        }

        segments.sort_unstable_by_key(|&(group, quality, _)| (group, quality));

        let stride = self.instance_stride;
        let raw = &self.instance_raw;
        for &(group, quality, ref segment_range) in segments.iter() {
            if draws.last().map(|draw| (draw.group_index, draw.quality)) != Some((group, quality)) {
                draws.push(DecoratorDraw {
                    group_index: group,
                    quality,
                    offset: compacted.len(),
                    instance_count: 0,
                });
            }

            let byte_start = segment_range.start as usize * stride;
            let byte_end = segment_range.end as usize * stride;
            if byte_start >= byte_end || byte_end > raw.len() {
                continue;
            }

            compacted.extend_from_slice(&raw[byte_start..byte_end]);
            if let Some(draw) = draws.last_mut() {
                draw.instance_count += segment_range.len() as u32;
            }
        }
    }

    fn populate_submit_node_blocks(
        &self,
        renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        _frame_node: &RenderPerFrameNode,
        visibility: &ViewVisibility,
        submit_node_blocks: &mut crate::renderer::packet::SubmitNodeContainer,
    ) {
        let stages = self.subscribed_stages() & visibility.render_stages;
        if stages.is_empty() || self.view_data.draws.is_empty() {
            *self.uploaded.write() = None;
            return;
        }

        let Some(slice) = renderer
            .gpu
            .frame()
            .upload
            .upload_bytes(&self.view_data.compacted)
        else {
            error!("Failed to upload decorator instance data (UploadHeap OOM)");
            *self.uploaded.write() = None;
            return;
        };

        *self.uploaded.write() = Some((slice.buffer(), slice.offset()));
        submit_node_blocks.broadcast(
            stages,
            crate::renderer::packet::SubmitNode { view_node, key: 0 },
        );
    }

    fn submit(
        &self,
        cmd: &mut crate::gpu::command_list::CommandList,
        stage: deimos_data::tfx::RenderStage,
        _frame_node: &crate::renderer::packet::RenderPerFrameNode,
        _view_node: &crate::renderer::packet::RenderPerViewNode,
        _submit_key: u64,
    ) {
        let Some((instance_buffer, instance_buffer_offset)) = *self.uploaded.read() else {
            return;
        };

        let speedtree_vertex_slot = 0xA; // renderer.globals.scopes.speedtree.vertex_slot() as u32;

        {
            let consts = &self.data.unk48.unk14;
            cmd.externs.speedtree_placements = Box::new(externs::SpeedtreePlacements {
                unk10: Vec4::W,
                unk20: consts.instances_scale,
                unk30: consts.instances_offset,
                unk40: consts.unk20,
                unk50: consts.unk30,
                unk60: consts.unk40,
                unk70: consts.unk50,
                // unk40: Default::default(),
                // unk50: Default::default(),
                // unk60: Default::default(),
                ..(cmd
                    .externs
                    .speedtree_placements
                    .as_deref()
                    .cloned()
                    .unwrap_or_default())
            })
            .into();
        }

        for draw in &self.view_data.draws {
            let model_id = if self.models.len() == 1 {
                0
            } else {
                draw.group_index as usize
            };

            let Some((model, ext, cb)) = self.models.get(model_id) else {
                warn!(
                    decorator_set = %self.hash,
                    mesh_count = self.models[0].0.model.meshes.len(),
                    variant_count = self.models[0].0.variant_count(),
                    identifier_count = self.models[0].0.identifier_count(),
                    "Decorator model index {} out of bounds for models list of length {}",
                    draw.group_index,
                    self.models.len(),
                );
                continue;
            };

            if let Some(cb) = cb {
                cb.bind_cbv(cmd, ShaderStage::Vertex, speedtree_vertex_slot);
            } else {
                cmd.set_shader_constant_buffer_view(
                    ShaderStage::Vertex,
                    speedtree_vertex_slot,
                    None,
                );
            }

            cmd.externs.rigid_model = Box::clone(ext).into();

            let dyn_id = if self.models.len() == 1 {
                draw.group_index
            } else {
                // cohae: Multi-models (usually trees) seem to use the ID as a LOD level?
                0
            };

            self.instance_blend_indices_vb.bind_single(cmd, 3);
            let buffer_offset = (instance_buffer_offset + draw.offset) as u64;
            model.draw_wrapped(
                cmd,
                stage,
                dyn_id,
                None,
                0,
                move |_model, cmd, _mesh, part| {
                    // LOD selector. 0=high, 1=medium, 2=low
                    if part.unk17 != draw.quality {
                        return;
                    }

                    cmd.set_vertex_buffers(1, &[instance_buffer], &[buffer_offset]);
                    cmd.draw_indexed(part.index_range(), 0..draw.instance_count, 0);
                },
            );
        }
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        self.models
            .iter()
            .fold(RenderStageSubscription::empty(), |acc, (model, _, _)| {
                acc | model.subscribed_stages
            })
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
