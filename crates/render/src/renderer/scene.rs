//! This module provides view and frame packet management for the renderer.
//!
//! You can think of it as a "sub-renderer" that can exist alongside other sub-renderers and the main renderer without state mucking.

use std::sync::Arc;

use ash::vk;
use deimos_data::{
    hash::fnv1,
    tfx::{
        FixedFunctionState, PrimitiveType, RenderStage, features::dynamic::RenderStageSubscription,
    },
};
use glam::{Vec3, Vec4};
use rayon::prelude::*;
use slotmap::SlotMap;

use crate::{
    ecs::{node_visibility_test, populate_submit_nodes},
    gpu::{
        Gpu,
        command_buffer::{
            IMAGE_STATE_COLOR_ATTACHMENT, IMAGE_STATE_DEPTH_ATTACHMENT, IMAGE_STATE_SHADER_READ,
            IMAGE_STATE_TRANSFER_DST, IMAGE_STATE_TRANSFER_SRC,
        },
        command_list::{
            ColorLoadOp, CommandList, CommandListForkState, DepthLoadOp, DepthMode, StoreOp,
        },
    },
    renderer::{
        Renderer,
        object::{RenderObject, RenderObjectHandle},
        packet::{FramePacket, RenderPerFrameNode, RenderPerViewNode, SubmitNode, ViewPacket},
        ssr::{SsrConstants, SsrRenderer},
    },
    tfx::view::ShadedView,
    visibility::ViewVisibility,
};

pub struct SceneRenderer {
    pub parent: Arc<Renderer>,

    pub frame_packet: FramePacket,

    pub main_view: ShadedView,
    pub ssr: SsrRenderer,

    pub global_channels: [Vec4; 256],
}

impl SceneRenderer {
    pub fn new(parent: Arc<Renderer>) -> anyhow::Result<Self> {
        let main_view = ShadedView::new(&parent.gpu, (1920, 1080))?;
        let ssr = SsrRenderer::new(&parent.gpu, main_view.shaded_read.output_format())?;

        let mut r = Self {
            frame_packet: FramePacket::default(),
            main_view,
            ssr,
            global_channels: parent.globals.channels.default_values(),
            parent,
        };

        r.set_global_channel_by_name("cubemap_sky_intensity", Vec4::splat(0.4));
        r.set_global_channel_by_name("global_cubemap_intensity", Vec4::splat(0.4));

        r.set_global_channel_by_name("global_ambient_intensity", Vec4::splat(5.0));
        r.set_global_channel_by_name("sun_direct_intensity", Vec4::splat(1.3));
        r.set_global_channel_by_id(0xE16B6B6B, Vec4::splat(1000000.0));
        r.set_global_channel_by_id(0x462A7037, Vec4::splat(0.0)); // >0 causes moss to glow?
        r.set_global_channel_by_id(0xCF70AC7C, Vec4::splat(0.0)); // something about transmission
        r.set_global_channel_by_id(0x07806276, Vec4::splat(0.8)); // fixes missing lights, but turning it up also dims transparent objects

        r.set_global_channel_by_id(0x2C538179, Vec4::splat(0.1)); // dims some very bright sky objects
        r.set_global_channel_by_id(0x2E538443, Vec4::splat(1.0)); // fixes marathon/planet rings not appearing

        // Controls cubemap reflectivity
        r.set_global_channel_by_id(0xCF70AC7C, Vec4::splat(0.0));
        r.set_global_channel_by_id(0x471E28F2, Vec4::splat(0.5));

        // Fixes some deferred lights not showing up
        r.set_global_channel_by_id(0xECA86879, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687A, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687B, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687C, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687D, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687E, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA8687F, Vec4::splat(1.0));
        r.set_global_channel_by_id(0xECA86870, Vec4::splat(1.0));

        Ok(r)
    }

    pub fn render(
        &mut self,
        cmd: &mut CommandList,
        visibility: &ViewVisibility,
        debug_pipeline: Option<DebugPipeline>,
        draw_sun_shadows: bool,
    ) {
        self.parent.externs.reset_global_channel_frequencies();
        let gpu = cmd.gpu().clone();
        // let stream = &gpu.frame().stream;

        self.parent.globals.scopes.frame.bind(cmd);
        self.parent.globals.scopes.chunk_model.bind(cmd);

        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "prepare_per_frame");

            let frame_packet = &self.frame_packet;
            let render_objects = self.parent.objects.read();

            for frame_node in &frame_packet.per_frame_nodes {
                let obj = &render_objects[frame_node.object];
                obj.renderer.prepare_per_frame(cmd, frame_node);
            }
        }

        if draw_sun_shadows && debug_pipeline.is_none_or(|d| d.needs_sun_shadows()) {
            self.render_shadow_views(&gpu, cmd);
        } else {
            for view in &mut self.main_view.shadow_views {
                view.depth.clear(cmd, 1.0, 0);
            }
        }

        self.render_main_view(&gpu, cmd, visibility, debug_pipeline);
    }

    fn render_main_view(
        &mut self,
        gpu: &Arc<Gpu>,
        cmd: &mut CommandList,
        visibility: &ViewVisibility,

        debug_pipeline: Option<DebugPipeline>,
    ) {
        let _scope = gpu.profiler_scope(cmd, "render_main_view");
        self.parent.externs.bump_generation();

        {
            let ext = self.parent.externs.get_mut();
            ext.view.world_to_camera = self.main_view.world_to_camera;
            ext.view.camera_to_projective = self.main_view.camera_to_projective;
            ext.view.derive_matrices(self.main_view.output.resolution());
        }
        self.parent.globals.scopes.view.bind(cmd);

        let use_hdri = debug_pipeline == Some(DebugPipeline::LookDev);

        self.main_view
            .gbuffer
            .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);
        self.main_view
            .gbuffer
            .depth
            .transition(cmd, IMAGE_STATE_DEPTH_ATTACHMENT);

        #[rustfmt::skip]
        cmd.begin_rendering_rt(
            &[
                Some((&self.main_view.gbuffer.albedo, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
                Some((&self.main_view.gbuffer.normal, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
                Some((&self.main_view.gbuffer.rt3, ColorLoadOp::Clear([0.0, 0.5, 0.0, 0.0]), StoreOp::Store)),
                Some((&self.main_view.gbuffer.rt4, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
            ],
            Some((&self.main_view.gbuffer.depth, DepthLoadOp::Clear(0.0, 0xFF), StoreOp::Store)),
            vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS
        );

        {
            let _scope = gpu.profiler_scope(cmd, "node_visibility_test");
            node_visibility_test(self, visibility, ShadedView::MAIN_VIEW_ID);
        }

        {
            let _scope = gpu.profiler_scope(cmd, "populate_submit_nodes");
            populate_submit_nodes(self, visibility, ShadedView::MAIN_VIEW_ID);
        }

        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "prepare_per_view");

            let frame_packet = &self.frame_packet;
            let render_objects = self.parent.objects.read();
            let view = &self.frame_packet.views[ShadedView::MAIN_VIEW_ID];
            for view_node in &view.view_nodes {
                let frame_node = &frame_packet.per_frame_nodes[view_node.frame_node];
                let obj = &render_objects[frame_node.object];
                obj.renderer.prepare_per_view(cmd, frame_node, view_node);
            }
        }

        let view = &self.frame_packet.views[ShadedView::MAIN_VIEW_ID];
        {
            let _scope = self
                .parent
                .gpu
                .profiler_scope(cmd, "submit_gbuffer_generation");

            cmd.set_ffstate(FixedFunctionState::new(Some(0), Some(2), Some(2), Some(0)));
            cmd.flush_states();
            self.submit_stage(cmd, view, RenderStage::GenerateGbuffer);
        }
        cmd.end_rendering();

        {
            self.main_view
                .gbuffer
                .uber_depth_half
                .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);
            // TODO(cohae): Uber depth generation uses compute shaders
            // cmd.set_render_targets(&[&self.main_view.gbuffer.uber_depth_half], None);
            self.main_view
                .gbuffer
                .uber_depth_half
                .clear(cmd, [0.0, 0.0, 0.0, 0.0]);

            self.parent.externs.get_mut().shadow_mask.unk10 =
                self.main_view.gbuffer.uber_depth_half.view().into();
            self.parent.externs.bump_generation();
            self.main_view
                .gbuffer
                .uber_depth_half
                .transition(cmd, IMAGE_STATE_SHADER_READ);
        }

        // Copy normals/depth buffer
        {
            let view = &mut self.main_view;

            // Copy normals
            view.gbuffer
                .normal
                .transition(cmd, IMAGE_STATE_TRANSFER_SRC);
            view.gbuffer
                .normal_read
                .transition(cmd, IMAGE_STATE_TRANSFER_DST);

            cmd.copy_render_target(&view.gbuffer.normal, &view.gbuffer.normal_read);
            view.gbuffer
                .normal_read
                .transition(cmd, IMAGE_STATE_SHADER_READ);
            view.gbuffer
                .normal
                .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);

            // Copy depth
            view.gbuffer.depth.transition(cmd, IMAGE_STATE_TRANSFER_SRC);
            view.gbuffer
                .depth_read
                .transition(cmd, IMAGE_STATE_TRANSFER_DST);

            cmd.copy_depth_buffer(&view.gbuffer.depth, &view.gbuffer.depth_read);

            view.gbuffer
                .depth_read
                .transition(cmd, IMAGE_STATE_SHADER_READ);
            view.gbuffer
                .depth
                .transition(cmd, IMAGE_STATE_DEPTH_ATTACHMENT);
        }

        #[rustfmt::skip]
        cmd.begin_rendering_rt(
            &[
                Some((&self.main_view.gbuffer.albedo, ColorLoadOp::Load, StoreOp::Store)),
                Some((&self.main_view.gbuffer.normal, ColorLoadOp::Load, StoreOp::Store)),
                Some((&self.main_view.gbuffer.rt3, ColorLoadOp::Load, StoreOp::Store)),
                Some((&self.main_view.gbuffer.rt4, ColorLoadOp::Load, StoreOp::Store)),
            ],
            Some((&self.main_view.gbuffer.depth, DepthLoadOp::Load, StoreOp::Store)),
            vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS
        );
        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "submit_decals");
            self.parent.globals.scopes.decal.bind(cmd);
            cmd.set_ffstate(FixedFunctionState::new(Some(8), Some(15), Some(2), Some(1)));
            cmd.flush_states();
            self.submit_stage(cmd, view, RenderStage::Decals);
        }

        {
            let _scope = self
                .parent
                .gpu
                .profiler_scope(cmd, "submit_decals_additive");

            cmd.set_ffstate(FixedFunctionState::new(Some(8), Some(15), Some(2), Some(1)));
            cmd.flush_states();
            self.submit_stage(cmd, view, RenderStage::DecalsAdditive);
        }
        cmd.end_rendering();

        self.main_view
            .gbuffer
            .transition(cmd, IMAGE_STATE_SHADER_READ);
        self.main_view
            .light
            .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);

        for cascade in &mut self.main_view.shadow_views {
            cascade.depth.transition(cmd, IMAGE_STATE_SHADER_READ);
        }

        let shadow_cascades = self
            .main_view
            .shadow_views
            .iter()
            .map(|cascade| (cascade.world_to_projective(), cascade.depth.depth_view()))
            .collect::<Vec<_>>();

        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "submit_cubemaps");

            #[rustfmt::skip]
            cmd.begin_rendering_rt(
                &[
                    Some((&self.main_view.light.light_diffuse, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
                    Some((&self.main_view.light.light_specular_ibl, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
                ],
                None,
                vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS
            );

            cmd.set_ffstate(FixedFunctionState::new(Some(23), Some(1), Some(3), Some(1)));
            cmd.flush_states();
            self.submit_stage(cmd, view, RenderStage::Cubemaps);

            if use_hdri {
                self.parent.apply_hdri_light(cmd, &shadow_cascades);
            }

            cmd.end_rendering();
        }

        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "submit_lighting_apply");

            cmd.set_render_target_formats(
                &[
                    self.main_view.light.light_diffuse.output_format(),
                    self.main_view.light.light_specular.output_format(),
                ],
                None,
            );

            #[rustfmt::skip]
            cmd.begin_rendering_rt(
                &[
                    Some((&self.main_view.light.light_diffuse, ColorLoadOp::Load, StoreOp::Store)),
                    Some((&self.main_view.light.light_specular, ColorLoadOp::Clear([0.0, 0.0, 0.0, 0.0]), StoreOp::Store)),
                ],
                None,
                vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS
            );

            cmd.set_ffstate(FixedFunctionState::new(Some(2), None, Some(2), Some(2)));
            cmd.flush_states();
            self.submit_stage(cmd, view, RenderStage::LightingApply);
            cmd.end_rendering();
        }

        if debug_pipeline.is_none_or(|d| d.needs_sun_shadows()) {
            self.main_view
                .shadow_mask
                .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);
            cmd.begin_rendering_rt(
                &[Some((
                    &self.main_view.shadow_mask,
                    ColorLoadOp::Load,
                    StoreOp::Store,
                ))],
                None,
                vk::RenderingFlags::empty(),
            );
            self.parent.generate_shadow_mask(cmd, &shadow_cascades);
            self.parent.externs.get_mut().shadow_mask.unk00 =
                self.main_view.shadow_mask.view().into();
            cmd.end_rendering();
            self.main_view
                .shadow_mask
                .transition(cmd, IMAGE_STATE_SHADER_READ);
        }

        self.main_view
            .light
            .transition(cmd, IMAGE_STATE_SHADER_READ);
        self.main_view
            .output
            .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);

        cmd.set_ffstate(FixedFunctionState::new(Some(0), Some(15), Some(1), Some(1)));

        if let Some(debug_pipeline) = debug_pipeline {
            #[rustfmt::skip]
            cmd.begin_rendering_rt(
                &[
                    Some((&self.main_view.output, ColorLoadOp::Clear([0.0, 0.0, 0.0, 1.0]), StoreOp::Store)),
                ],
                None,
                vk::RenderingFlags::empty()
            );

            let p = &self.parent.globals.pipelines;
            let technique = match debug_pipeline {
                DebugPipeline::LookDev => &p.deferred_shading,
                DebugPipeline::GlobalLightingShading => &p.global_lighting_and_shading,
                DebugPipeline::DeferredShading => &p.deferred_shading,
                DebugPipeline::DeferredShadingNoAtm => &p.deferred_shading_no_atm,
                DebugPipeline::Albedo => &p.debug_source_color,
                DebugPipeline::Smoothness => &p.debug_specular_smoothness,
                DebugPipeline::Metalness => &p.debug_metalness,
                DebugPipeline::AmbientOcclusion => &p.debug_ambient_occlusion,
                DebugPipeline::Emission => &p.debug_emissive,
                DebugPipeline::EmissionIntensity => &p.debug_emissive_intensity,
                DebugPipeline::Transmission => &p.debug_transmission,
                DebugPipeline::Overcoat => &p.debug_colored_overcoat_id,
                DebugPipeline::DepthEdges => &p.debug_depth_edges,
                DebugPipeline::WorldNormal => &p.debug_world_normal,
                DebugPipeline::LightDiffuse => &p.debug_diffuse_light,
                DebugPipeline::LightSpecular => &p.debug_specular_light,

                DebugPipeline::Overdraw => &p.global_lighting_and_shading,
            };

            self.parent
                .execute_global_pipeline(cmd, technique, &format!("{debug_pipeline:?}"));
            cmd.end_rendering();
        } else {
            self.main_view
                .gbuffer
                .albedo
                .transition(cmd, IMAGE_STATE_TRANSFER_SRC);
            self.main_view
                .output
                .transition(cmd, IMAGE_STATE_TRANSFER_DST);

            cmd.copy_render_target(&self.main_view.gbuffer.albedo, &self.main_view.output);
        }

        if use_hdri {
            cmd.begin_rendering_rt(
                &[Some((
                    &self.main_view.output,
                    ColorLoadOp::Load,
                    StoreOp::Store,
                ))],
                None,
                vk::RenderingFlags::empty(),
            );
            self.parent.draw_hdri_background(cmd);
            cmd.end_rendering();
        }

        if self.ssr.settings.enabled
            && debug_pipeline
                .is_some_and(|pipeline| pipeline.is_shaded() || pipeline == DebugPipeline::LookDev)
        {
            let _scope = self.parent.gpu.profiler_scope(cmd, "submit_ssr");

            let constants = SsrConstants::new(
                self.main_view.world_to_camera,
                self.main_view.camera_to_projective,
                self.parent.externs.view.target_pixel_to_camera,
                self.main_view.gbuffer.albedo.resolution(),
                &self.ssr.settings,
            );

            self.main_view
                .output
                .transition(cmd, IMAGE_STATE_SHADER_READ);
            self.main_view
                .shaded_read
                .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);

            cmd.set_ffstate(FixedFunctionState::new(Some(0), Some(15), Some(1), Some(1)));
            cmd.flush_states();
            cmd.set_input_topology(PrimitiveType::TriangleStrip);
            cmd.set_input_layout(0);

            cmd.begin_rendering_rt(
                &[Some((
                    &self.main_view.shaded_read,
                    ColorLoadOp::DontCare,
                    StoreOp::Store,
                ))],
                None,
                vk::RenderingFlags::empty(),
            );

            if let Err(e) = self.ssr.submit(
                cmd,
                &constants,
                self.main_view.output.view(),
                self.main_view.gbuffer.normal.view(),
                self.main_view.gbuffer.depth_read.depth_view(),
                self.main_view.gbuffer.rt3.view(),
            ) {
                error!("Failed to submit SSR pass: {e:?}");
            }

            cmd.end_rendering();

            self.main_view
                .output
                .transition(cmd, IMAGE_STATE_TRANSFER_DST);
            self.main_view
                .shaded_read
                .transition(cmd, IMAGE_STATE_TRANSFER_SRC);
            cmd.copy_render_target(&self.main_view.shaded_read, &self.main_view.output);
        }

        self.main_view
            .output
            .transition(cmd, IMAGE_STATE_TRANSFER_SRC);
        self.main_view
            .shaded_read
            .transition(cmd, IMAGE_STATE_TRANSFER_DST);
        cmd.copy_render_target(&self.main_view.output, &self.main_view.shaded_read);

        self.main_view
            .output
            .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);
        self.main_view
            .light
            .distortion
            .transition(cmd, IMAGE_STATE_COLOR_ATTACHMENT);

        #[rustfmt::skip]
        cmd.begin_rendering_rt(
            &[
                Some((&self.main_view.output, ColorLoadOp::Load, StoreOp::Store)),
                None,
                None,
                None,
                Some((&self.main_view.light.distortion, ColorLoadOp::Load, StoreOp::Store,)),
            ],
            Some((&self.main_view.gbuffer.depth, DepthLoadOp::Load, StoreOp::None,)),
            vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS,
        );

        {
            self.parent.globals.scopes.transparent.bind(cmd);
            self.parent.globals.scopes.transparent_advanced.bind(cmd);
            let _scope = self.parent.gpu.profiler_scope(cmd, "submit_transparents");
            cmd.set_ffstate(FixedFunctionState::new(Some(8), Some(15), Some(2), Some(1)));
            self.submit_stage(cmd, view, RenderStage::Transparents);

            // self.parent.immediate.draw_shapes(cmd);
        }
        cmd.end_rendering();

        self.main_view
            .output
            .transition(cmd, IMAGE_STATE_SHADER_READ);
    }

    fn render_shadow_views(&mut self, gpu: &Arc<Gpu>, cmd: &mut CommandList) {
        cmd.set_depth_mode(DepthMode::Forward);
        let _scope = gpu.profiler_scope(cmd, "render_shadow_views");

        let num_shadow_views = self.main_view.shadow_views.len();
        for shadow_view_index in 0..num_shadow_views {
            let _scope = gpu.profiler_scope(cmd, "render_shadow_view");
            self.parent.externs.bump_generation();

            let (visibility, view_id) = {
                let shadow_view = &self.main_view.shadow_views[shadow_view_index];

                let ext = self.parent.externs.get_mut();
                ext.view.world_to_camera = shadow_view.world_to_camera;
                ext.view.camera_to_projective = shadow_view.camera_to_projective;
                ext.view.derive_matrices(shadow_view.depth.resolution());

                (
                    ViewVisibility {
                        enabled: true,
                        position: Vec3::ZERO,
                        far_plane: 500.0,
                        culling_frustum: shadow_view.frustum.clone(),
                        world_to_projective: ext.view.world_to_projective,
                        render_stages: RenderStageSubscription::SHADOW_VIEW,
                        occlusion_buffer: None,
                        visible_cluster_bounds: None,
                    },
                    shadow_view.id,
                )
            };

            self.parent.globals.scopes.view.bind(cmd);

            {
                let _scope = gpu.profiler_scope(cmd, "node_visibility_test");
                node_visibility_test(self, &visibility, view_id);
            }

            {
                let _scope = gpu.profiler_scope(cmd, "populate_submit_nodes");
                populate_submit_nodes(self, &visibility, view_id);
            }

            let shadow_view = &mut self.main_view.shadow_views[shadow_view_index];
            let view = &self.frame_packet.views[shadow_view.id];
            {
                let _scope = gpu.profiler_scope(cmd, "prepare_per_view");

                let render_objects = self.parent.objects.read();
                for view_node in &view.view_nodes {
                    let frame_node = &self.frame_packet.per_frame_nodes[view_node.frame_node];
                    let obj = &render_objects[frame_node.object];
                    obj.renderer.prepare_per_view(cmd, frame_node, view_node);
                }
            }

            shadow_view
                .depth
                .transition(cmd, IMAGE_STATE_DEPTH_ATTACHMENT);
            {
                let _scope = self
                    .parent
                    .gpu
                    .profiler_scope(cmd, "submit_shadow_generate");

                cmd.begin_rendering_rt(
                    &[],
                    Some((
                        &shadow_view.depth,
                        DepthLoadOp::Clear(1.0, 0),
                        StoreOp::Store,
                    )),
                    vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS,
                );

                cmd.set_ffstate(FixedFunctionState::new(Some(0), Some(2), Some(2), Some(0)));
                cmd.flush_states();
                self.submit_stage(cmd, view, RenderStage::ShadowGenerate);
                cmd.end_rendering();
            }

            self.main_view.shadow_views[shadow_view_index]
                .depth
                .transition(cmd, IMAGE_STATE_SHADER_READ);
        }

        cmd.set_depth_mode(DepthMode::Reverse);
    }

    pub fn submit_stage(&self, cmd: &mut CommandList, view: &ViewPacket, stage: RenderStage) {
        let frame_packet = &self.frame_packet;
        let render_objects = self.parent.objects.read();
        let submit_nodes = view.submit_node_blocks.block(stage);

        if submit_nodes.is_empty() {
            return;
        }

        let _stage_scope = cmd.event_scope(stage.to_string(), [0.0, 1.0, 0.0]);

        let fork = cmd.fork_state();
        let gpu = &self.parent.gpu;
        let frame_nodes = frame_packet.per_frame_nodes.as_slice();
        let view_nodes = view.view_nodes.as_slice();
        let render_objects: &SlotMap<RenderObjectHandle, RenderObject> = &render_objects;

        const MIN_CHUNK_SIZE: usize = 8;

        let thread_count = rayon::current_num_threads().max(1);
        let chunk_size = submit_nodes
            .len()
            .div_ceil(thread_count)
            .max(MIN_CHUNK_SIZE);

        let command_buffers = submit_nodes
            .par_chunks(chunk_size)
            .map(|chunk| {
                record_submit_chunk(
                    gpu,
                    &fork,
                    frame_nodes,
                    view_nodes,
                    render_objects,
                    stage,
                    chunk,
                )
            })
            .collect::<Vec<_>>();

        let command_buffers = command_buffers.into_iter().flatten().collect::<Vec<_>>();
        cmd.execute_commands(&command_buffers);
    }

    /// Sets the value of the given global channel by ID
    /// Returns `Some` with the previous value if the channel exists, `None` otherwise
    pub fn set_global_channel_by_id(&mut self, id: u32, v: Vec4) -> Option<Vec4> {
        if let Some(pos) = self.parent.externs.global_ids.iter().position(|i| *i == id) {
            Some(std::mem::replace(&mut self.global_channels[pos], v))
        } else {
            None
        }
    }

    /// Sets the value of the given global channel by name, hashing the name to get its ID
    /// Returns `Some` with the previous value if the channel exists, `None` otherwise
    pub fn set_global_channel_by_name(&mut self, name: &str, v: Vec4) -> Option<Vec4> {
        self.set_global_channel_by_id(fnv1(name), v)
    }

    pub fn get_global_channel_by_name(&self, name: &str) -> Option<Vec4> {
        self.get_global_channel_by_id(fnv1(name))
    }

    pub fn get_global_channel_by_id(&self, id: u32) -> Option<Vec4> {
        self.parent
            .externs
            .global_ids
            .iter()
            .position(|i| *i == id)
            .map(|pos| self.global_channels[pos])
    }
}

#[allow(clippy::too_many_arguments)]
fn record_submit_chunk(
    gpu: &Arc<Gpu>,
    fork: &CommandListForkState,
    frame_nodes: &[RenderPerFrameNode],
    view_nodes: &[RenderPerViewNode],
    render_objects: &SlotMap<RenderObjectHandle, RenderObject>,
    stage: RenderStage,
    submit_nodes: &[SubmitNode],
) -> Option<vk::CommandBuffer> {
    let cmd = match gpu.acquire_secondary_command_buffer(fork.color_formats(), fork.depth_format())
    {
        Ok(Some(cmd)) => cmd,
        Ok(None) => {
            error!("Out of secondary command buffers while submitting {stage}");
            return None;
        }
        Err(e) => {
            error!("Failed to begin secondary command buffer for {stage}: {e:?}");
            return None;
        }
    };

    let mut cmd = CommandList::from_fork_state(gpu, cmd, fork.clone());

    for submit_node in submit_nodes {
        let Some(view_node) = view_nodes.get(submit_node.view_node) else {
            error!("Submit node has invalid view node index");
            continue;
        };
        let Some(frame_node) = frame_nodes.get(view_node.frame_node) else {
            error!("View node has invalid frame node index");
            continue;
        };
        let Some(render_object) = render_objects.get(frame_node.object) else {
            error!(
                "Render object with handle {:?} not found",
                frame_node.object
            );
            continue;
        };
        let _obj_scope = cmd.event_scope(&render_object.name, [0.2, 0.2, 1.0]);
        render_object
            .renderer
            .submit(&mut cmd, stage, frame_node, view_node, submit_node.key);
    }

    let command_buffer = *cmd.cmd;
    if let Err(e) = gpu.end_secondary_command_buffer(command_buffer) {
        error!("Failed to end secondary command buffer for {stage}: {e:?}");
        return None;
    }

    Some(command_buffer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugPipeline {
    LookDev,
    GlobalLightingShading,
    DeferredShading,
    DeferredShadingNoAtm,

    Albedo,
    Smoothness,
    Metalness,
    AmbientOcclusion,
    Emission,
    EmissionIntensity,
    Transmission,
    Overcoat,

    DepthEdges,
    WorldNormal,
    Overdraw,

    LightDiffuse,
    LightSpecular,
}

impl DebugPipeline {
    pub const fn is_shaded(&self) -> bool {
        matches!(
            self,
            Self::GlobalLightingShading | Self::DeferredShading | Self::DeferredShadingNoAtm
        )
    }

    pub const fn needs_sun_shadows(&self) -> bool {
        matches!(
            self,
            Self::LookDev
                | Self::GlobalLightingShading
                | Self::DeferredShadingNoAtm
                | Self::LightDiffuse
                | Self::LightSpecular
        )
    }

    pub const fn aa_enabled(&self) -> bool {
        self.is_shaded() || matches!(self, Self::DepthEdges | Self::WorldNormal)
    }
}
