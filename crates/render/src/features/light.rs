use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{
    ExternIndex, PrimitiveType, RenderStage, ShaderStage,
    features::{
        dynamic::RenderStageSubscription,
        light::{SLight, SShadowingLight},
    },
    geometry::AxisAlignedBBox,
};
use deimos_ecs::transform::Transform;
use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use tiger_pkg::TagHash;

use crate::{
    asset::{index_buffer::IndexBuffer, vertex_buffer::VertexBuffer},
    camera::CameraProjection,
    features::dynamic_model::DynamicObjectData,
    gpu::render_target::DepthBuffer,
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode},
    },
    tfx::{
        externs::{self, BaseExternSource},
        technique::Technique,
    },
    util::geometry,
};

use super::FeatureRenderer;

pub const SHADOWMAP_RESOLUTION: u32 = 1024;

#[derive(Clone, Copy)]
pub struct ShadowCamera {
    pub world_to_camera: Mat4,
    pub camera_to_projective: Mat4,
    pub far_plane: f32,
}

pub struct LightRenderer {
    technique_lighting_apply: Box<Technique>,
    technique_lighting_apply_shadowing: Option<Box<Technique>>,
    // technique_volumetrics: Option<Technique>,
    // technique_light_probe_apply: Technique,

    // TODO(cohae): This should be a shared resource (eg. a struct in the renderer that we can use instead of recreating it for every light/cubemap)
    vb: VertexBuffer,
    ib: IndexBuffer,

    unk50: Vec4,
    light_space_transform: glam::Mat4,
    bounds: Option<AxisAlignedBBox>,

    shadowmap_projection: Mat4,
    shadow_camera: Option<ShadowCamera>,
    shadow_map: Option<DepthBuffer>,
}

impl LightRenderer {
    pub fn new(
        renderer: &Renderer,
        light: &SLight,
        bounds: AxisAlignedBBox,
    ) -> anyhow::Result<Box<Self>> {
        Self::new_impl(
            renderer,
            light.technique_lighting_apply,
            TagHash::NONE,
            light.technique_volumetrics,
            // light.technique_light_probe_apply,
            light.unk50,
            light.light_space_transform,
            None,
            Some(bounds),
        )
    }

    pub fn new_shadowing(
        renderer: &Renderer,
        light: &SShadowingLight,
        transform: Transform,
    ) -> anyhow::Result<Box<Self>> {
        let shadow_camera = ShadowCamera {
            world_to_camera: transform.view_matrix(),
            camera_to_projective: CameraProjection::Perspective.matrix(
                1.0,
                (light.half_fov * 2.0).to_degrees(),
                0.5,
                light.far_plane,
            ),
            far_plane: light.far_plane,
        };

        Self::new_impl(
            renderer,
            light.technique_lighting_apply,
            light.technique_lighting_apply_shadowing,
            light.technique_volumetrics,
            light.unk50,
            light.light_space_transform,
            Some(shadow_camera),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_impl(
        renderer: &Renderer,
        technique_shading: TagHash,
        technique_shading_shadowing: TagHash,
        _technique_volumetrics: TagHash,
        // technique_volumetrics_shadowing: TagHash,
        // technique_light_probe: TagHash,
        // technique_light_probe_shadowing: TagHash,
        unk50: Vec4,
        light_space_transform: Mat4,
        shadow_camera: Option<ShadowCamera>,
        bounds: Option<AxisAlignedBBox>,
    ) -> anyhow::Result<Box<Self>> {
        let vb = VertexBuffer::load_data(
            &renderer.gpu,
            bytemuck::cast_slice(geometry::CUBE_VERTICES),
            size_of::<Vec3>() as u32,
        )?;
        let ib = IndexBuffer::load_data(
            &renderer.gpu,
            bytemuck::cast_slice(geometry::CUBE_INDICES),
            false,
        )?;

        Ok(Box::new(Self {
            technique_lighting_apply: Technique::load(
                &renderer.asset_manager,
                &renderer.gpu,
                technique_shading,
            )
            .context("load technique lighting_apply")?,
            technique_lighting_apply_shadowing: technique_shading_shadowing
                .is_some()
                .then(|| {
                    Technique::load(
                        &renderer.asset_manager,
                        &renderer.gpu,
                        technique_shading_shadowing,
                    )
                    .context("load technique lighting_apply_shadowing")
                })
                .transpose()?,
            // technique_volumetrics: technique_volumetrics
            //     .is_some()
            //     .then(|| {
            //         Technique::load(
            //             &renderer.asset_manager,
            //             &renderer.gpu,
            //             technique_volumetrics,
            //         )
            //         .context("load technique volumetrics")
            //     })
            //     .transpose()?,
            // technique_light_probe_apply: Technique::load(&renderer.gpu, technique_light_probe)?,
            vb,
            ib,
            unk50,
            light_space_transform,
            bounds,
            shadowmap_projection: shadow_camera.map_or(Mat4::IDENTITY, |c| c.camera_to_projective),
            shadow_camera,
            shadow_map: None,
        }))
    }

    /// Returns the shadow map if it needs to be registered with a shadow view.
    pub const fn pending_shadow_camera(&self) -> Option<ShadowCamera> {
        if self.shadow_map.is_some() {
            return None;
        }

        self.shadow_camera
    }

    pub fn set_shadow_view(&mut self, depth: DepthBuffer) {
        self.shadow_map = Some(depth);
    }

    pub fn calculate_bounds(&mut self) -> AxisAlignedBBox {
        let points = geometry::CUBE_VERTICES
            .iter()
            .map(|&v| self.light_space_transform.project_point3(v))
            .collect::<Vec<_>>();
        let bb = AxisAlignedBBox::from_points(&points);
        self.bounds = Some(bb);
        bb
    }
}

impl FeatureRenderer for LightRenderer {
    // fn prepare_per_frame(&self, cmd: &mut CommandList, frame_node: &RenderPerFrameNode) {
    //     let data = unsafe { frame_node.data::<DynamicObjectData>() }
    //         .expect("DynamicModel was extracted with no data!");

    //     self.local_to_world = data.local_to_world;

    //     let local_to_world_scaled = self.local_to_world * self.light_space_transform;
    //     let points = geometry::CUBE_VERTICES
    //         .iter()
    //         .map(|&v| local_to_world_scaled.project_point3(v))
    //         .collect_vec();

    //     self.bounds = Some(AxisAlignedBBox::from_points(&points));
    // }

    // fn visibility_test(&mut self, camera: &Camera) -> bool {
    //     if let Some(ref bounds) = self.bounds {
    //         camera.is_visible(bounds)
    //     } else {
    //         true
    //     }
    // }

    // fn extract_and_prepare(
    //     &mut self,
    //     _renderer: &crate::Renderer,
    //     extracted_data: &dyn std::any::Any,
    // ) {
    //     // TODO(cohae): lights shouldnt need to extract permutations at all
    //     let (obj_local_to_world, _permutation) = extracted_data
    //         .downcast_ref::<(CompactTransform, usize)>()
    //         .expect("Invalid extracted data type")
    //         .clone();

    //     self.local_to_world = obj_local_to_world.to_mat4();

    //     let local_to_world_scaled = self.local_to_world * self.light_space_transform;
    //     let points = geometry::CUBE_VERTICES
    //         .iter()
    //         .map(|&v| local_to_world_scaled.project_point3(v))
    //         .collect_vec();

    //     self.bounds = Some(AxisAlignedBBox::from_points(&points));
    // }

    // fn submit(&self, cmd: &mut crate::gpu::command_list::CommandList, stage: RenderStage) {
    //     if stage != RenderStage::LightingApply {
    //         // TODO
    //         return;
    //     }

    //     {
    //         // let (scale, _rotation, _translation) =
    //         //     self.local_to_world.to_scale_rotation_translation();

    //         let local_to_world_scaled = self.local_to_world * self.light_space_transform;
    //         let externs = Renderer::instance().externs.get();
    //         cmd.externs.simple_geometry = Some(Box::new(SimpleGeometry {
    //             local_to_world: externs.view.world_to_projective
    //                 * local_to_world_scaled
    //                 * Mat4::from_scale(Vec3::NEG_ONE),
    //         }));

    //         let view_translation_inverse_mat4 = Mat4::from_translation(-externs.view.position());
    //         let local_to_world_relative = view_translation_inverse_mat4 * self.local_to_world;

    //         let (min, max) = compute_light_bounds(self.light_space_transform);
    //         let light_local_to_world = compute_light_local_to_world(self.local_to_world, min, max);

    //         cmd.externs.deferred_light = Some(Box::new(DeferredLight {
    //             // unk40: local_to_world_relative.inverse().transpose(),
    //             unk40: (view_translation_inverse_mat4 * light_local_to_world).inverse(),
    //             unkc0: local_to_world_relative,

    //             unk150: 1.0,
    //             unk154: 0.0,
    //             unk158: 0.0,

    //             ..Default::default()
    //         }));

    //         cmd.externs.dynamic_model = Some(Box::new(externs::RigidModel {
    //             local_to_world: light_local_to_world,
    //             ..Default::default()
    //         }));
    //     }

    //     self.technique_lighting_apply.bind(cmd).unwrap();

    //     cmd.set_input_topology(PrimitiveType::Triangles);
    //     cmd.set_input_layout(1); // float3 v0 : POSITION0, // Format DXGI_FORMAT_R32G32B32_FLOAT size 12

    //     cmd.input_assembler_set_index_buffer(&self.ib, dxgi::Format::R16Uint, 0);
    //     cmd.input_assembler_set_vertex_buffers(0, &[Some(&self.vb)], Some(&[12]), Some(&[0]))
    //         .unwrap();

    //     cmd.draw_indexed(geometry::CUBE_INDICES.len() as u32, 0, 0);
    //     cmd.flush_states();
    // }

    fn populate_submit_node_blocks(
        &self,
        _renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        _frame_node: &RenderPerFrameNode,
        visibility: &crate::visibility::ViewVisibility,
        submit_node_blocks: &mut crate::renderer::packet::SubmitNodeContainer,
    ) {
        submit_node_blocks.broadcast(
            self.subscribed_stages() & visibility.render_stages,
            crate::renderer::packet::SubmitNode { view_node, key: 0 },
        );
    }

    fn submit(
        &self,
        cmd: &mut crate::gpu::command_list::CommandList,
        stage: deimos_data::tfx::RenderStage,
        frame_node: &crate::renderer::packet::RenderPerFrameNode,
        _view_node: &crate::renderer::packet::RenderPerViewNode,
        _submit_key: u64,
    ) {
        if stage != RenderStage::LightingApply {
            // TODO
            return;
        }
        let data = unsafe { frame_node.data::<DynamicObjectData>() }
            .expect("DynamicModel was extracted with no data!");

        let transform_relative;
        {
            // let (scale, _rotation, _translation) =
            //     self.local_to_world.to_scale_rotation_translation();

            let local_to_world_scaled = data.local_to_world * self.light_space_transform;
            let renderer = match cmd.externs.base() {
                BaseExternSource::Renderer(renderer) => renderer.clone(),
                BaseExternSource::None => {
                    error!("LightRenderer: No renderer found in externs base");
                    return;
                }
            };
            let view = &renderer.externs.view;
            cmd.externs.simple_geometry = Some(Box::new(externs::SimpleGeometry {
                local_to_world: view.world_to_projective
                    * local_to_world_scaled
                    * Mat4::from_scale(Vec3::NEG_ONE),
            }));

            let view_translation_inverse_mat4 = Mat4::from_translation(-view.position());
            let local_to_world_relative = view_translation_inverse_mat4 * data.local_to_world;

            let (min, max) = compute_light_bounds(self.light_space_transform);
            let light_local_to_world = compute_light_local_to_world(data.local_to_world, min, max);

            let (_, transform_rot, transform_translation) =
                data.local_to_world.to_scale_rotation_translation();
            let forward = transform_rot * Vec3::X;
            let up = transform_rot * Vec3::Z;
            let transform_translation = transform_translation - view.position();
            transform_relative =
                Mat4::look_at_rh(transform_translation, transform_translation + forward, up);

            cmd.externs.deferred_light = Some(Box::new(externs::DeferredLight {
                // unk40: local_to_world_relative.inverse().transpose(),
                unk40: (view_translation_inverse_mat4 * light_local_to_world).inverse(),
                unkc0: local_to_world_relative,

                unk140: self.unk50,
                unk150: 1.0,
                unk154: 1.0,
                unk158: 1.0,
                unk160: 1.0,

                ..Default::default()
            }));

            cmd.externs.rigid_model = Some(Box::new(externs::RigidModel {
                local_to_world: light_local_to_world,
                ..Default::default()
            }));
        }

        cmd.set_input_topology(PrimitiveType::Triangles);
        cmd.set_input_layout(1); // float3 v0 : POSITION0, // Format DXGI_FORMAT_R32G32B32_FLOAT size 12

        self.vb.bind_single(cmd, 0);
        self.ib.bind(cmd);

        // TODO(cohae): Unknown what this texture is supposed to be. VS loads the first pixel and uses it as multiplier for the shadowmap UVs
        if let Some(depth) = &self.shadow_map {
            let shadowmap_vs_t2 = cmd
                .gpu()
                .magic_textures
                .get_texture(
                    cmd.gpu(),
                    (ExternIndex::DeferredShadow, 255), // TODO(cohae): DeferredShadow->_0xFF is not a real extern, this should be a proper global texture
                    Some(([0, 0, 0, 0], 4, 4, 1)),
                )
                .unwrap_or_else(|e| {
                    error!("Failed to create fallback texture for descriptor: {e:?}");
                    vk::ImageView::null()
                });
            cmd.set_shader_resource_view(ShaderStage::Vertex, 2, Some(shadowmap_vs_t2));

            let existing_shadowmap = cmd
                .externs
                .deferred_shadow
                .as_ref()
                .cloned()
                .unwrap_or_default();

            cmd.externs.deferred_shadow = Some(
                externs::DeferredShadow {
                    shadow_depthmap: depth.depth_view().into(),
                    resolution_width: depth.resolution().0 as f32,
                    resolution_height: depth.resolution().1 as f32,
                    unkc0: self.shadowmap_projection * transform_relative,
                    unk180: 1.0,
                    ..*existing_shadowmap
                }
                .into(),
            );

            self.technique_lighting_apply_shadowing
                .as_ref()
                .unwrap_or(&self.technique_lighting_apply)
                .bind(cmd);
        } else {
            self.technique_lighting_apply.bind(cmd);
        }

        cmd.draw_indexed(0..geometry::CUBE_INDICES.len() as u32, 0..1, 0);
        cmd.flush_states();
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        RenderStageSubscription::LIGHTING_APPLY
            | RenderStageSubscription::LIGHT_PROBE_APPLY
            | RenderStageSubscription::VOLUMETRICS
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn compute_light_bounds(light_space_transform: Mat4) -> (Vec3, Vec3) {
    let mut points = [
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, 1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];

    for point in &mut points {
        let p = light_space_transform.mul_vec4(point.extend(1.0));
        let point_w_abs = (-p.wwww()).abs();
        *point = Vec4::select(
            point_w_abs.cmpge(Vec4::splat(0.0001)),
            p / p.wwww(),
            Vec4::W,
        )
        .truncate();
    }

    points
        .iter()
        .fold((Vec3::MAX, Vec3::MIN), |(min, max), &point| {
            (min.min(point), max.max(point))
        })
}

fn compute_light_local_to_world(node_local_to_world: Mat4, min: Vec3, max: Vec3) -> Mat4 {
    let bounds_center = min.midpoint(max);
    let bounds_half_extents = (max - min) / 2.0;

    // First matrix operation ("mat"):
    // Each column is computed by scaling one of node_local_to_world’s axes by the corresponding component of bounds_half_extents,
    // except for the w-axis which is a linear combination of the x, y, and z axes plus the original w-axis.
    let mat = Mat4 {
        x_axis: node_local_to_world.x_axis * bounds_half_extents.x,
        y_axis: node_local_to_world.y_axis * bounds_half_extents.y,
        z_axis: node_local_to_world.z_axis * bounds_half_extents.z,
        w_axis: node_local_to_world.x_axis * bounds_center.x
            + node_local_to_world.y_axis * bounds_center.y
            + node_local_to_world.z_axis * bounds_center.z
            + node_local_to_world.w_axis,
    };

    // Second matrix operation ("mat_scaled"):
    // Scale the x, y, and z axes by 2, and subtract all three from the w-axis.
    let mat_scaled = Mat4 {
        x_axis: mat.x_axis * 2.0,
        y_axis: mat.y_axis * 2.0,
        z_axis: mat.z_axis * 2.0,
        w_axis: mat.w_axis - mat.x_axis - mat.y_axis - mat.z_axis,
    };

    // Third matrix operation (computing light_local_to_world):
    // Rearrange the columns of mat_scaled: swap the x and z axes, leaving y and w unchanged.

    Mat4 {
        x_axis: mat_scaled.z_axis,
        y_axis: mat_scaled.y_axis,
        z_axis: mat_scaled.x_axis,
        w_axis: mat_scaled.w_axis,
    }
}
