use ahash::AHashMap;
use ash::vk;
use deimos_data::tfx::{
    RenderStage, ShaderStage, TfxScopeBits,
    features::dynamic::{
        RenderStageSubscription, SDynamicMesh, SDynamicMeshMaterialVariants, SDynamicMeshPart,
        SDynamicModel,
    },
};
use deimos_ecs::object::ObjectChannel;
use glam::{Mat4, UVec4, Vec4, Vec4Swizzles};
use itertools::{Itertools, multizip};
use tiger_parse::PackageManagerExt;
use tiger_pkg::TagHash;
use tiger_pkg::package_manager;

use crate::{
    asset::{Handle, handle::is_technique_loaded, vertex_buffer::VertexBuffer},
    features::{FeatureRenderer, skinning},
    gpu::{
        buffer::ImmutableBuffer,
        command_list::{CommandList, ConstantBufferView},
    },
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode, SubmitNode, SubmitNodeContainer},
    },
    tfx::technique::Technique,
};

use super::shared::ModelBuffers;

pub struct DynamicModel {
    mesh_buffers: Vec<(ModelBuffers, ImmutableBuffer, ImmutableBuffer)>,

    technique_map: Vec<SDynamicMeshMaterialVariants>,
    techniques: Vec<Handle<Technique>>,

    pub model: SDynamicModel,
    pub scale_factor_1d: f32,
    pub mesh_stages: Vec<RenderStageSubscription>,
    pub subscribed_stages: RenderStageSubscription,
    part_techniques: Vec<Vec<Handle<Technique>>>,

    // pub selected_mesh: usize,
    pub default_permutation: usize,
    pub permutation_count: usize,

    identifier_count: usize,

    pub hash: TagHash,

    pub channels: AHashMap<u32, ObjectChannel>,
    pub transform: Mat4,
}

impl DynamicModel {
    #[profiling::function]
    pub fn load(
        renderer: &Renderer,
        hash: TagHash,
        technique_map: Vec<SDynamicMeshMaterialVariants>,
        techniques: Vec<TagHash>,
    ) -> anyhow::Result<Box<Self>> {
        let model = package_manager().read_tag_struct::<SDynamicModel>(hash)?;

        let techniques = techniques
            .iter()
            .map(|&tag| renderer.asset_manager.load(tag))
            .collect_vec();

        let scale_factor_1d = model.model_scale.xyz().max_element();
        let mesh_buffers = model
            .meshes
            .iter()
            .map(|m| {
                let (v0_data, v0_stride) = VertexBuffer::get_raw_data_and_stride(m.vertex0_buffer)
                    .expect("Failed to load vertex0 data");

                let skinning_posdata = skinning::repack_pos_words_from_vb(
                    &v0_data,
                    v0_stride as usize,
                    model.model_scale.xyz(),
                    scale_factor_1d,
                )
                .expect("Failed to repack position data");

                let skinning_normaldata =
                    skinning::repack_normal_tangent_words_from_vb(&v0_data, v0_stride as usize)
                        .expect("Failed to repack position data");

                let skinning_posbuffer = ImmutableBuffer::new(
                    &renderer.gpu,
                    "skinning_posbuffer",
                    vk::Format::R32_UINT,
                    bytemuck::cast_slice(&skinning_posdata),
                )
                .expect("Failed to create skinning position buffer");

                let skinning_normalbuffer = ImmutableBuffer::new(
                    &renderer.gpu,
                    "skinning_normalbuffer",
                    vk::Format::R32_UINT,
                    bytemuck::cast_slice(&skinning_normaldata),
                )
                .expect("Failed to create skinning normal buffer");

                (
                    ModelBuffers::load(
                        renderer,
                        m.vertex0_buffer,
                        m.vertex1_buffer,
                        m.index_buffer,
                        m.color_buffer,
                    )
                    .expect("Failed to load model buffers for dynamic model"),
                    skinning_posbuffer,
                    skinning_normalbuffer,
                )
            })
            .collect_vec();

        let mesh_stages = model
            .meshes
            .iter()
            .map(|m| RenderStageSubscription::from_partrange_list(&m.part_range_per_render_stage))
            .collect_vec();

        let part_techniques = model
            .meshes
            .iter()
            .map(|m| {
                m.parts
                    .iter()
                    .map(|p| renderer.asset_manager.load(p.technique))
                    .collect_vec()
            })
            .collect_vec();

        let permutation_count = technique_map
            .iter()
            .filter(|m| m.unk8 == 0)
            .map(|m| m.technique_count as usize)
            .next()
            .unwrap_or(1);

        let identifier_count = model
            .meshes
            .iter()
            .map(|m| {
                m.parts
                    .iter()
                    .map(|p| p.external_identifier)
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0) as usize
            + 1;

        Ok(Box::new(Self {
            default_permutation: if permutation_count > 2 {
                permutation_count - 1
            } else {
                0
            },
            permutation_count,
            // selected_mesh: 0,
            identifier_count,
            mesh_buffers,
            technique_map,
            techniques,
            model,
            scale_factor_1d,
            subscribed_stages: mesh_stages
                .iter()
                .fold(RenderStageSubscription::empty(), |acc, &x| acc | x),
            mesh_stages,
            part_techniques,
            hash,
            channels: AHashMap::default(),
            transform: Mat4::IDENTITY,
        }))
    }

    pub const fn mesh_count(&self) -> usize {
        self.model.meshes.len()
    }

    pub const fn variant_count(&self) -> usize {
        self.permutation_count
    }

    pub const fn identifier_count(&self) -> usize {
        self.identifier_count
    }

    fn get_permutation_technique(
        &self,
        index: u16,
        permutation_count: usize,
    ) -> Option<Handle<Technique>> {
        if index == u16::MAX {
            None
        } else {
            self.technique_map
                .get(index as usize)
                .as_ref()
                .map(|permutation_range| {
                    self.techniques[permutation_range.technique_start as usize
                        + (permutation_count % permutation_range.technique_count as usize)]
                        .clone()
                })
        }
    }

    // /// ⚠ Expects the `rigid_model` scope to be bound
    // pub fn draw(
    //     &self,
    //     renderer: &Renderer,
    //     render_stage: TfxRenderStage,
    //     identifier: u16,
    //     object_channels: Option<&ObjectChannels>,
    // ) -> anyhow::Result<()> {
    //     self.draw_wrapped(
    //         renderer,
    //         render_stage,
    //         identifier,
    //         object_channels,
    //         |_, renderer, _mesh, part| unsafe {
    //             renderer
    //                 .gpu
    //                 .lock_context()
    //                 .DrawIndexed(part.index_count, part.index_start, 0);
    //         },
    //     )
    // }

    pub fn draw_wrapped<F>(
        &self,
        cmd: &mut CommandList,
        stage: RenderStage,
        identifier: u16,
        permutation: Option<usize>,
        mut f: F,
    ) where
        F: FnMut(&Self, &mut CommandList, &SDynamicMesh, &SDynamicMeshPart),
    {
        for (
            mesh,
            subscribed_stages,
            (mesh_buffers, skinning_posbuffer, skinning_normalbuffer),
            mesh_techniques,
        ) in multizip((
            self.model.meshes.iter(),
            self.mesh_stages.iter(),
            self.mesh_buffers.iter(),
            self.part_techniques.iter(),
        )) {
            if !subscribed_stages.is_subscribed(stage) {
                continue;
            }

            // self.cb.bind(cmd, ShaderStage::Vertex, 1);
            // self.cb.bind(cmd, ShaderStage::Pixel, 1);

            cmd.set_input_layout(mesh.get_input_layout_for_stage(stage) as usize);
            mesh_buffers.bind(cmd);
            skinning_posbuffer.bind_srv(cmd, ShaderStage::Vertex, 2);
            skinning_normalbuffer.bind_srv(cmd, ShaderStage::Vertex, 3);
            cmd.set_shader_resource_view(ShaderStage::Vertex, 4, None);

            for part_index in mesh.get_range_for_stage(stage) {
                let part = &mesh.parts[part_index];
                if identifier != u16::MAX && part.external_identifier != identifier {
                    continue;
                }

                if !part.lod_category.is_highest_detail() {
                    continue;
                }

                let variant_material = self.get_permutation_technique(
                    part.variant_shader_index,
                    permutation.unwrap_or(self.default_permutation),
                );

                let mut all_scopes = TfxScopeBits::empty();
                if let Some(technique) = mesh_techniques[part_index].get() {
                    technique.bind_with_channels(cmd, Some(&self.channels));
                    // technique
                    //     .bind_with_channels(cmd, Some(&self.channels))
                    //     .expect("Failed to bind technique");
                    all_scopes |= technique.data.used_scopes;
                }

                if let Some(technique) = &variant_material
                    && let Some(technique) = technique.get()
                {
                    technique.bind_with_channels(cmd, Some(&self.channels));
                    // tech.bind_with_channels(cmd, Some(&self.channels))
                    //     .expect("Failed to bind variant technique");
                    all_scopes |= technique.data.used_scopes;
                }

                // No technique, no scopes, no draw
                if all_scopes.is_empty() {
                    continue;
                }

                // TODO(cohae): Reimplement in pipeline cache
                // if all_scopes.contains(TfxScopeBits::SKINNING) {
                //     cmd.vertex_set_shader(&Renderer::instance().common.disable_skinning_vs);
                // }

                cmd.set_input_topology(part.primitive_type);

                f(self, cmd, mesh, part);
            }
        }
    }
}

impl FeatureRenderer for DynamicModel {
    // fn extract(&mut self, _renderer: &Renderer, data: &dyn std::any::Any) {
    //     let (obj_local_to_world, permutation) = *data
    //         .downcast_ref::<(Mat4, usize)>()
    //         .expect("Invalid extracted data type");
    //     self.transform = obj_local_to_world;
    //     self.permutation = permutation;

    //     self.constants = RigidModelConstants {
    //         mesh_to_world: obj_local_to_world,
    //         position_scale: self.model.model_scale,
    //         position_offset: self.model.model_offset,
    //         texcoord0_scale_offset: Vec4::new(
    //             self.model.texcoord_scale.x,
    //             self.model.texcoord_scale.y,
    //             self.model.texcoord_offset.x,
    //             self.model.texcoord_offset.y,
    //         ),
    //         dynamic_sh_ao_values: Vec4::new(0.0, 0.0, 0.0, 0.8),
    //     };
    // }

    // fn prepare(&mut self, _renderer: &Renderer) {}

    // fn submit(&self, cmd: &mut CommandList, stage: RenderStage) {
    //     profiling::scope!("DynamicModel::draw");

    //     self.draw_wrapped(cmd, stage, u16::MAX, |_model, cmd, _mesh, part| {
    //         cmd.draw_indexed_instanced(part.index_range(), 0..1, 0);
    //     });
    // }

    fn prepare_per_frame(&self, cmd: &mut CommandList, frame_node: &RenderPerFrameNode) {
        let data = unsafe { frame_node.data::<DynamicObjectData>() }
            .expect("DynamicModel was extracted with no data!");

        let rigid_model_cb = match cmd.gpu().frame().upload.alloc::<RigidModelConstants>() {
            Ok(o) => o,
            Err(e) => {
                error!("Failed to allocate rigid model constants: {e:?}");
                return;
            }
        };

        let scale_and_offset = self.model.model_offset.with_w(self.scale_factor_1d);
        rigid_model_cb.write(&RigidModelConstants {
            mesh_to_world: data.local_to_world,
            position_scale: self.model.model_scale,
            position_offset: self.model.model_offset,
            texcoord0_scale_offset: Vec4::new(
                self.model.texcoord_scale.x,
                self.model.texcoord_scale.y,
                self.model.texcoord_offset.x,
                self.model.texcoord_offset.y,
            ),
            dynamic_sh_ao_values: Vec4::new(0.0, 0.0, 0.0, 0.8),
            skinning: SkinningConstants {
                unk12: scale_and_offset,
                unk13: scale_and_offset,
                ..Default::default()
            },
        });

        data.cbuffer = ConstantBufferView {
            addr: rigid_model_cb.virtual_address(),
            size: size_of::<RigidModelConstants>() as u64,
        };
    }

    #[profiling::function]
    fn submit(
        &self,
        cmd: &mut CommandList,
        stage: RenderStage,
        frame_node: &RenderPerFrameNode,
        _view_node: &RenderPerViewNode,
        _submit_key: u64,
    ) {
        cmd.disable_smart_technique_binding();
        let data = unsafe { frame_node.data::<DynamicObjectData>() }
            .expect("DynamicModel was extracted with no data!");

        cmd.set_shader_constant_buffer_view(ShaderStage::Vertex, 1, Some(data.cbuffer));
        cmd.set_shader_constant_buffer_view(ShaderStage::Pixel, 1, Some(data.cbuffer));

        self.draw_wrapped(
            cmd,
            stage,
            u16::MAX,
            Some(data.permutation),
            |_model, cmd, _mesh, part| {
                cmd.draw_indexed_instanced(part.index_range(), 0..1, 0);
            },
        );
    }

    fn populate_submit_node_blocks(
        &self,
        _renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        frame_node: &RenderPerFrameNode,
        visibility: &crate::visibility::ViewVisibility,
        submit_node_blocks: &mut SubmitNodeContainer,
    ) {
        let data = unsafe { frame_node.data::<DynamicObjectData>() }
            .expect("DynamicModel was extracted with no data!");

        let (_, _, translation) = data.local_to_world.to_scale_rotation_translation();

        let distance = translation.distance(visibility.position);
        // reversed so further objects are rendered first
        let distance_normalized = 1.0 - (distance as f64 / visibility.far_plane as f64);
        let distance_u32 = (distance_normalized.clamp(0.0, 1.0) * u32::MAX as f64) as u32;

        submit_node_blocks.broadcast(
            self.subscribed_stages & visibility.render_stages,
            SubmitNode {
                view_node,
                key: distance_u32 as u64,
            },
        );
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        self.subscribed_stages
    }

    fn is_loaded(&self) -> bool {
        if self
            .part_techniques
            .iter()
            .any(|v| v.iter().any(|t| !is_technique_loaded(t)))
        {
            return false;
        }

        if self.techniques.iter().any(|t| !is_technique_loaded(t)) {
            return false;
        }

        if self.mesh_buffers.iter().any(|(m, _, _)| !m.is_loaded()) {
            return false;
        }

        true
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[repr(C)]
pub struct RigidModelConstants {
    mesh_to_world: Mat4,          // c0-c3
    position_scale: Vec4,         // c4
    position_offset: Vec4,        // c5
    texcoord0_scale_offset: Vec4, // c6
    dynamic_sh_ao_values: Vec4,   // c7
    skinning: SkinningConstants,
}

#[repr(C)]
pub struct SkinningConstants {
    unk8: Mat4,   // c8
    unk12: Vec4,  // c12
    unk13: Vec4,  // c13
    unk14: UVec4, // c14
    unk15: Mat4,  // c15
}

impl Default for SkinningConstants {
    fn default() -> Self {
        Self {
            unk8: Mat4::IDENTITY,
            unk12: Vec4::W,
            unk13: Vec4::W,
            unk14: UVec4::ZERO, // X is for t2/t3 offset, Y for t4
            unk15: Mat4::IDENTITY,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DynamicObjectData {
    pub local_to_world: Mat4,
    pub permutation: usize,
    pub cbuffer: ConstantBufferView,
    pub _pad: u64,
}
