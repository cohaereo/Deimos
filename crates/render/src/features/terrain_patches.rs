use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{
    RenderStage, ShaderStage,
    features::{
        dynamic::RenderStageSubscription,
        terrain::{STerrain, TerrainDetailLevel},
    },
    geometry::AxisAlignedBBox,
};
use glam::Vec4;
use itertools::Itertools;
use tiger_parse::PackageManagerExt;
use tiger_pkg::TagHash;
use tiger_pkg::package_manager;

use crate::{
    asset::{Handle, index_buffer::IndexBuffer, texture::Texture, vertex_buffer::VertexBuffer},
    gpu::{buffer::ImmutableBuffer, command_list::CommandList},
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode, SubmitNode, SubmitNodeContainer},
    },
    tfx::technique::Technique,
    visibility::ViewVisibility,
};

use super::FeatureRenderer;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug, Pod, Zeroable)]
pub struct TerrainPatchGroupConstants {
    offset: Vec4,
    texcoord_transform: Vec4,
    unk20: f32,
    unk24: f32,
    unk28: f32,
    ao_offset: u32,
    unk30: Vec4,
}

pub struct TerrainPatchesRenderer {
    terrain: STerrain,
    techniques: Vec<Handle<Technique>>,
    dyemaps: Vec<Handle<Texture>>,
    group_cbuffers: Vec<ImmutableBuffer>,
    detail_level: TerrainDetailLevel,

    technique_shadow: Handle<Technique>,
    technique_depth_only: Handle<Technique>,

    pub vertex0_buffer: Handle<VertexBuffer>,
    pub vertex1_buffer: Handle<VertexBuffer>,
    pub index_buffer: Handle<IndexBuffer>,

    pub hash: TagHash,
    pub identifier: u64,
}

impl TerrainPatchesRenderer {
    pub fn load(renderer: &Renderer, hash: TagHash, identifier: u64) -> anyhow::Result<Box<Self>> {
        let terrain: STerrain = package_manager().read_tag_struct(hash)?;

        let assets = &renderer.asset_manager;
        let dyemaps = terrain
            .mesh_groups
            .iter()
            .map(|group| assets.load(group.dyemap))
            .collect();

        let techniques = terrain
            .mesh_parts
            .iter()
            .map(|part| assets.load(part.technique))
            .collect_vec();

        let group_cbuffers = terrain
            .mesh_groups
            .iter()
            .map(|group| {
                let offset = Vec4::new(
                    terrain.unk30.x,
                    terrain.unk30.y,
                    terrain.unk30.z,
                    terrain.unk30.w,
                );

                let texcoord_transform =
                    Vec4::new(group.unk20.x, group.unk20.y, group.unk20.z, group.unk20.w);

                // let scope_terrain = Mat4::from_cols(offset, texcoord_transform, Vec4::ZERO, Vec4::ZERO);
                let scope_terrain = TerrainPatchGroupConstants {
                    offset,
                    texcoord_transform,
                    ao_offset: 0x02000000,
                    // ao_offset: ao
                    //     .and_then(|ao| ao.get_offset_by_identifier(self.identifier))
                    //     .unwrap_or(0x02000000),
                    ..Default::default()
                };

                ImmutableBuffer::new(
                    &renderer.gpu,
                    "terrain_patch_constants",
                    vk::Format::R32G32B32A32_UINT,
                    bytemuck::cast_slice(&[scope_terrain]),
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .context("allocating terrain patch constant buffers")?;

        Ok(Box::new(Self {
            vertex0_buffer: assets.load(terrain.vertex0_buffer),
            vertex1_buffer: assets.load(terrain.vertex1_buffer),
            index_buffer: assets.load(terrain.index_buffer),
            detail_level: TerrainDetailLevel::Medium,
            technique_depth_only: assets.load(terrain.technique_depth_only),
            technique_shadow: assets.load(terrain.technique_shadow),
            terrain,
            techniques,
            dyemaps,
            group_cbuffers,
            hash,
            identifier,
        }))
    }

    pub const fn bounds(&self) -> AxisAlignedBBox {
        self.terrain.bounds
    }

    #[profiling::function]
    pub fn render_group(
        &self,
        cmd: &mut CommandList,
        render_stage: RenderStage,
        group_index: usize,
    ) {
        // cmd.enable_smart_technique_binding();
        // gpu_event!(renderer.gpu, format!("terrain_patch {}", self.hash));
        // gpu_span!();

        // Layout 22(tfs/goliath)/60(sk)
        //  - int4 v0 : POSITION0, // Format DXGI_FORMAT_R16G16B16A16_SINT size 8
        //  - float4 v1 : NORMAL0, // Format DXGI_FORMAT_R16G16B16A16_SNORM size 8
        //  - float2 v2 : TEXCOORD1, // Format DXGI_FORMAT_R16G16_FLOAT size 4
        cmd.set_input_layout(22);
        cmd.set_input_topology(deimos_data::tfx::PrimitiveType::TriangleStrip);

        if let (Some(vertex0), Some(vertex1), Some(index)) = (
            self.vertex0_buffer.get(),
            self.vertex1_buffer.get(),
            self.index_buffer.get(),
        ) {
            index.bind(cmd);
            cmd.set_vertex_buffers(
                0,
                &[vertex0.buffer().vk_buffer(), vertex1.buffer().vk_buffer()],
                &[0, 0],
            );
        } else {
            return;
        }

        let Some(part) = self.terrain.mesh_parts.get(group_index) else {
            return;
        };

        let constants = &self.group_cbuffers[part.group_index as usize];
        constants.bind_cbv(cmd, ShaderStage::Vertex, 11);

        if let Some(dyemap) = self.dyemaps[part.group_index as usize].get() {
            dyemap.bind(cmd, 14, ShaderStage::Pixel);
        }

        let technique = match render_stage {
            RenderStage::ShadowGenerate => &self.technique_shadow,
            RenderStage::DepthPrepass => &self.technique_depth_only,
            RenderStage::GenerateGbuffer => &self.techniques[group_index],
            _ => return,
        };

        if technique.get_ref(|technique| technique.bind(cmd)).is_none() {
            return;
        }

        cmd.draw_indexed_instanced(part.index_range(), 0..1, 0);
    }
}

impl FeatureRenderer for TerrainPatchesRenderer {
    // fn visibility_test(&mut self, camera: &Camera) -> bool {
    //     let center = self.terrain.bounds.center();
    //     let _radius = self.terrain.bounds.radius();
    //     let _distance = camera.position.distance(center);
    //     // self.detail_level = match distance {
    //     //     d if d > radius * 4.0 => TerrainDetailLevel::Low,
    //     //     d if d > radius * 2.0 => TerrainDetailLevel::Medium,
    //     //     _ => TerrainDetailLevel::High,
    //     // };

    //     camera
    //         .culling_frustum
    //         .aabb_intersecting(&self.terrain.bounds)
    // }

    fn submit(
        &self,
        cmd: &mut CommandList,
        stage: RenderStage,
        _frame_node: &RenderPerFrameNode,
        _view_node: &RenderPerViewNode,
        submit_key: u64,
    ) {
        self.render_group(cmd, stage, submit_key as usize);
    }

    #[profiling::function]
    fn populate_submit_node_blocks(
        &self,
        _renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        _frame_node: &RenderPerFrameNode,
        visibility: &ViewVisibility,
        submit_node_blocks: &mut SubmitNodeContainer,
    ) {
        for (i, _part) in self
            .terrain
            .mesh_parts
            .iter()
            .enumerate()
            .filter(|(_, u)| u.detail_level == self.detail_level)
        {
            submit_node_blocks.broadcast(
                self.subscribed_stages() & visibility.render_stages,
                SubmitNode {
                    view_node,
                    key: i as u64,
                },
            );
        }
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        RenderStageSubscription::GENERATE_GBUFFER
            | RenderStageSubscription::SHADOW_GENERATE
            | RenderStageSubscription::DEPTH_PREPASS
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
