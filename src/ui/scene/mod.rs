use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use ash::vk;
use deimos_data::tfx::{
    FeatureRendererSubscription, features::dynamic::RenderStageSubscription,
    geometry::AxisAlignedBBox,
};
use deimos_render::{
    camera::{Camera, CameraProjection},
    ecs::{s_extract_frame_packet, s_update_object_channels, s_visibility_test},
    features::decorators::DECORATOR_QUALITY,
    gpu::{command_list::CommandList, render_target::RenderTarget},
    renderer::{
        Renderer,
        cascades::CascadeCalculator,
        scene::{DebugPipeline, SceneRenderer},
    },
    tfx::{
        externs::{self, get_global_channel_name},
        scope::FrameScope,
    },
    visibility::{ViewVisibility, frustum::Frustum},
};
use egui::{
    Color32, FontId, Image, ImageSource, Rect, Response, RichText, Sense, TextStyle, Ui, UiBuilder,
    Vec2, Widget, containers::menu::MenuConfig, load::SizedTexture, vec2,
};
use glam::{Vec3, Vec4, Vec4Swizzles, vec3, vec4};
use google_material_symbols::GoogleMaterialSymbols;
use hecs::World;
use itertools::Itertools;
use umbra::QueryErrorCode;

use crate::{
    app::SharedState,
    ui::{
        GuiRenderer,
        scene::controller::CameraController,
        util::{ExternalDataWidgetExt, UiExt, format_bytes},
    },
};

pub mod controller;

pub struct Scene {
    renderer: Arc<Renderer>,
    pub scene_renderer: SceneRenderer,
    start_time: Instant,

    // Camera/view
    camera: Camera,
    pub controller: CameraController,
    subscribed_features: FeatureRendererSubscription,
    pub render_mode: RenderMode,
    draw_sun_shadows: bool,

    // World
    pub world: World,
    raininess: f32,
    heat_cascade: f32,
    time_of_day: f32,
    animate_time_of_day: bool,
    sun_light_angle: f32,

    // Metrics
    frametimes: Vec<f32>,
    last_frame_time: Instant,

    // UI
    keep_settings_open: bool,
    show_channel_editor: bool,
    only_show_used_channels: bool,

    umbra_enabled: bool,
    umbra_result: QueryErrorCode,
}

impl Scene {
    pub fn new(
        renderer: &Arc<Renderer>,
        shared: &SharedState,
        camera: Camera,
    ) -> anyhow::Result<Self> {
        let mut scene_renderer = SceneRenderer::new(renderer.clone())?;
        scene_renderer
            .main_view
            .set_resolution_scale(shared.config.read().resolution_scale);

        Ok(Self {
            scene_renderer,
            renderer: renderer.clone(),
            start_time: Instant::now(),
            camera,
            controller: CameraController::new_first_person(),
            subscribed_features: FeatureRendererSubscription::all()
                .difference(FeatureRendererSubscription::SPEEDTREE_TREES),
            render_mode: RenderMode::Lookdev,
            draw_sun_shadows: false,

            world: World::new(),
            raininess: 0.0,
            heat_cascade: 0.0,
            time_of_day: 1200.0,
            animate_time_of_day: false,
            sun_light_angle: 60f32,

            frametimes: Vec::new(),
            last_frame_time: Instant::now(),
            keep_settings_open: false,
            show_channel_editor: false,
            only_show_used_channels: true,
            umbra_result: QueryErrorCode::Ok,
            umbra_enabled: true,
        })
    }

    pub const fn with_controller(mut self, controller: CameraController) -> Self {
        self.controller = controller;
        self
    }

    pub const fn with_render_mode(mut self, render_mode: RenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }

    pub fn set_world(&mut self, world: World) {
        self.world = world;
    }

    pub fn take_world(&mut self) -> hecs::World {
        std::mem::take(&mut self.world)
    }

    pub fn clear(&mut self) {
        self.world.clear();
    }
}

impl Scene {
    pub fn show(&mut self, ui: &mut egui::Ui, renderer: &mut GuiRenderer, size: Vec2) {
        let now = Instant::now();
        let delta_time = (now - self.last_frame_time).as_secs_f32();
        self.frametimes.push(delta_time);
        while self.frametimes.len() > 60 {
            self.frametimes.remove(0);
        }

        self.last_frame_time = now;
        let delta_time_average = if !self.frametimes.is_empty() {
            let sum: f32 = self.frametimes.iter().sum();
            sum / self.frametimes.len() as f32
        } else {
            delta_time
        };

        // if self.show_surface_viewer {
        //     egui::SidePanel::right("surface_viewer").show_inside(ui, |ui| {
        //         // self.show_texture_viewer(ui, egui_d3d11);
        //         self.show_surface_viewer(ui, egui_d3d11);
        //     });
        // }

        if self.show_channel_editor {
            egui::SidePanel::right("channel_editor").show_inside(ui, |ui| {
                self.show_channel_editor(ui);
            });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let panel_rect = ui.available_rect_before_wrap();

            // let r = ui.allocate_response(size, Sense::CLICK | Sense::DRAG | Sense::HOVER);
            let r = ui
                .image(SizedTexture {
                    id: renderer.add_user_texture(self.scene_renderer.main_view.output.view()),
                    size,
                })
                .interact(Sense::CLICK | Sense::DRAG | Sense::HOVER);

            if !ui.is_rect_visible(r.rect) {
                return;
            }

            let mut bar_rect = r.rect;
            bar_rect.set_height(32.0);
            ui.painter().rect_filled(
                bar_rect,
                0.0,
                egui::Color32::from_black_alpha(if ui.rect_contains_pointer(bar_rect) {
                    160
                } else {
                    64
                }),
            );
            ui.scope_builder(UiBuilder::new().max_rect(bar_rect), |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    self.show_toolbar(ui);
                })
            });

            let fps_rect = ui.painter_at(panel_rect).text(
                panel_rect.right_top() + Vec2::new(0.0, 3.0) + Vec2::splat(1.0),
                egui::Align2::RIGHT_TOP,
                format!("{} ", (1. / delta_time_average).round()),
                egui::FontId::monospace(16.0),
                egui::Color32::BLACK,
            );

            ui.painter_at(panel_rect).text(
                panel_rect.right_top() + Vec2::new(0.0, 3.0),
                egui::Align2::RIGHT_TOP,
                format!("{} ", (1. / delta_time_average).round()),
                egui::FontId::monospace(16.0),
                egui::Color32::GREEN,
            );

            let memory_stats = self.renderer.gpu.memory_stats();
            let mut vram_text = format_bytes(memory_stats.total_bytes_used as usize);
            let mut vram_color = Color32::GREEN;
            if !memory_stats.errors.is_empty() || memory_stats.high_water {
                vram_text = format!("{} {}", GoogleMaterialSymbols::Error, vram_text);
                vram_color = Color32::RED;
            } else if !memory_stats.warnings.is_empty() {
                vram_text = format!("{} {}", GoogleMaterialSymbols::Warning, vram_text);
                vram_color = Color32::YELLOW;
            }

            let vram_rect = ui.painter_at(panel_rect).text(
                panel_rect.right_top() + Vec2::new(-8.0, 23.0) + Vec2::splat(1.0),
                egui::Align2::RIGHT_TOP,
                &vram_text,
                egui::FontId::monospace(16.0),
                egui::Color32::BLACK,
            );

            ui.painter_at(panel_rect).text(
                panel_rect.right_top() + Vec2::new(-8.0, 23.0),
                egui::Align2::RIGHT_TOP,
                vram_text,
                egui::FontId::monospace(16.0),
                vram_color,
            );

            ui.scope_builder(
                egui::UiBuilder::new().max_rect(panel_rect.shrink2(vec2(12.0, 4.0))),
                |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        if let Some(last_speed_change) = ui.memory(|m| {
                            m.data.get_temp::<Instant>("scene_last_speed_change".into())
                        }) && last_speed_change.elapsed().as_secs_f32() <= 2.0
                        {
                            ui.label(format!(
                                "{} Camera Speed: {:.1}m/s",
                                GoogleMaterialSymbols::Speed,
                                self.controller.speed()
                            ));
                        }

                        if self.umbra_result == QueryErrorCode::OutsideScene {
                            ui.label(
                                RichText::new(format!(
                                    "{} Outside Scene",
                                    GoogleMaterialSymbols::Warning
                                ))
                                .color(Color32::YELLOW)
                                .size(16.0),
                            );
                        }

                        if self.world.is_empty() {
                            ui.label(
                                RichText::new(format!(
                                    "{} Scene is empty",
                                    GoogleMaterialSymbols::Warning
                                ))
                                .color(Color32::YELLOW)
                                .size(16.0),
                            );
                        }

                        if self.renderer.asset_manager.count_loading() > 0 {
                            ui.label(
                                RichText::new(format!(
                                    "{} Loading assets... ({} in progress)",
                                    GoogleMaterialSymbols::HardDrive,
                                    self.renderer.asset_manager.count_loading()
                                ))
                                .size(16.0),
                            );
                        }
                    });
                },
            );

            ui.style_mut().spacing.tooltip_width = 4096.0;
            // Renderer::instance().profiler.set_enabled(false);
            ui.interact(
                vram_rect,
                "vram_report_profiler_tooltip".into(),
                Sense::hover(),
            )
            .on_hover_ui(|ui| {
                ui.style_mut().spacing.item_spacing = vec2(0.0, 0.0);
                // ui.monospace(format!("{} allocations", memory_stats.num_allocations));
                ui.monospace(format!(
                    "Bytes Allocated: {}",
                    format_bytes(memory_stats.total_bytes_used as usize)
                ));
                // ui.monospace(format!(
                //     "Descriptor Heap: {}/{} ({:.0}%)",
                //     memory_stats.descriptor_heap_used,
                //     memory_stats.descriptor_heap_capacity,
                //     memory_stats.descriptor_heap_used as f32
                //         / memory_stats.descriptor_heap_capacity as f32
                //         * 100.0
                // ));
                ui.separator();
                ui.monospace(format!(
                    "Upload Ring:  {}/{} ({})",
                    format_bytes(memory_stats.upload_ring_used),
                    format_bytes(memory_stats.upload_ring_capacity),
                    memory_stats.num_upload_ring_allocations
                ));
                ui.capacity_bar(
                    memory_stats.upload_ring_capacity,
                    memory_stats.upload_ring_used,
                );
                ui.monospace(format!(
                    "Descriptor Ring:  {}/{} ({})",
                    format_bytes(memory_stats.descriptor_ring_used),
                    format_bytes(memory_stats.descriptor_ring_capacity),
                    memory_stats.num_descriptor_ring_allocations
                ));
                ui.capacity_bar(
                    memory_stats.descriptor_ring_capacity,
                    memory_stats.descriptor_ring_used,
                );

                ui.separator();

                // ui.monospace(format!(
                //     "Buffers: {}",
                //     format_bytes(
                //         self.renderer
                //             .gpu
                //             .num_bytes_allocated_for_buffers
                //             .load(Ordering::Relaxed)
                //     )
                // ));

                // ui.monospace(format!(
                //     "Upload/Download Buffers: {}",
                //     format_bytes(
                //         self.renderer
                //             .gpu
                //             .num_bytes_allocated_for_transfer_buffers
                //             .load(Ordering::Relaxed)
                //     )
                // ));

                // ui.monospace(format!(
                //     "Render Targets: {}",
                //     format_bytes(
                //         self.renderer
                //             .gpu
                //             .num_bytes_allocated_for_render_targets
                //             .load(Ordering::Relaxed)
                //     )
                // ));

                // ui.monospace(format!(
                //     "Textures: {}",
                //     format_bytes(
                //         self.renderer
                //             .gpu
                //             .num_bytes_allocated_for_textures
                //             .load(Ordering::Relaxed)
                //     )
                // ));

                if !memory_stats.errors.is_empty() {
                    ui.separator();
                    ui.colored_label(Color32::RED, "Errors:");
                    for error in memory_stats.errors.iter() {
                        ui.monospace(format!(" - {error:?}"));
                    }
                } else if !memory_stats.warnings.is_empty() {
                    ui.separator();
                    ui.colored_label(Color32::YELLOW, "Warnings:");
                    for warning in memory_stats.warnings.iter() {
                        ui.monospace(format!(" - {warning:?}"));
                    }
                }
            });

            let size_pixels = size * ui.ctx().pixels_per_point();
            let resolution = (size_pixels.x as u32, size_pixels.y as u32);

            self.controller.update(&mut self.camera, ui, &r, delta_time);

            if self.animate_time_of_day {
                self.time_of_day += delta_time;
                self.time_of_day = self.time_of_day.rem_euclid(3600.0);
            }

            if r.dragged_by(egui::PointerButton::Middle) {
                let delta_adjusted = r.drag_delta() / 4.0;
                self.sun_light_angle += delta_adjusted.x;
                self.sun_light_angle = self.sun_light_angle.rem_euclid(360.0);
            }

            let sun_light_direction = if self.render_mode == RenderMode::Lookdev {
                vec3(0.510, 0.317, 0.799).normalize()
            } else {
                vec3(
                    self.sun_light_angle.to_radians().cos(),
                    self.sun_light_angle.to_radians().sin(),
                    0.7,
                )
                .normalize()
            };

            self.scene_renderer
                .set_global_channel_by_name("sun_light_direction", sun_light_direction.extend(0.0));

            self.scene_renderer
                .set_global_channel_by_id(0x156C2B22, Vec4::splat(self.raininess));
            self.scene_renderer
                .set_global_channel_by_id(0xD8281393, Vec4::splat(self.raininess));
            self.scene_renderer
                .set_global_channel_by_id(0xCF70AC7C, Vec4::splat(self.raininess));
            self.scene_renderer
                .set_global_channel_by_id(0x471E28F2, Vec4::splat(self.raininess));

            self.scene_renderer
                .set_global_channel_by_id(0x2C53817A, Vec4::splat(1.0 - self.raininess));
            self.scene_renderer
                .set_global_channel_by_id(0xFDCC7BAA, Vec4::splat(self.heat_cascade));

            // for (e, (transform, source)) in self
            //     .world
            //     .query::<With<(&Transform, &PatternSource), &TempAudioMarker>>()
            //     .iter()
            // {
            //     if transform.translation.distance(self.camera.position) < 16.0 {
            //         println!("Near audio pattern {}", source.tag);
            //     }
            // }
            // println!("---------------------------------------");

            self.render(delta_time, resolution);

            ui.interact(
                fps_rect,
                "frame_counter_profiler_tooltip".into(),
                Sense::hover(),
            )
            .on_hover_ui(|ui| {
                let gpu = &self.renderer.gpu;
                ui.monospace(format!(
                    "{} Drawcalls",
                    gpu.num_drawcalls.load(Ordering::Relaxed)
                ));
                ui.monospace(format!(
                    "{} Static Instances",
                    gpu.num_static_instances.load(Ordering::Relaxed)
                ));
                let profiler_results = gpu.frame().profiler.get_results_string();
                ui.add(egui::Label::new(RichText::new(profiler_results).monospace()).extend());
            });
        });
    }

    #[profiling::function]
    pub fn render(&mut self, delta_time: f32, canvas_resolution: (u32, u32)) {
        let cmd = self.renderer.gpu.current_command_buffer();
        let mut cmd = CommandList::from_native_command_list(&self.renderer.gpu, cmd);
        let _scene_scope = self.renderer.gpu.profiler_scope(&cmd, "scene_render");
        s_update_object_channels(&self.world);
        if let Err(e) = self
            .scene_renderer
            .main_view
            .resize(&self.renderer.gpu, canvas_resolution)
        {
            error!("Failed to resize main view: {e:?}");
        }

        let resolution = self.scene_renderer.main_view.resolution();

        let _event = cmd.event_scope("Scene::render", [1.0, 1.0, 1.0]);

        let mut occlusion_buffer = None;
        let mut visible_cluster_bounds = None;
        if let Some((_, tome)) = self.world.query::<&umbra::Tome>().iter().next()
            && self.umbra_enabled
        {
            profiling::scope!("umbra_visibility");

            let mut query = umbra::Query::new(tome);
            let mut vis = umbra::Visibility::default();
            let mut ob = umbra::OcclusionBuffer::default();

            let mut cluster_storage = vec![0i32; tome.get_cluster_count() as usize];
            let mut clusters = umbra::IndexList::new(&mut cluster_storage);
            vis.set_output_clusters(&mut clusters);
            vis.set_output_buffer(&mut ob);

            self.umbra_result = query.query_portal_visibility(
                0,
                &vis,
                &umbra::CameraTransform::new(
                    (self.camera.projection_matrix_standard() * self.camera.view_matrix())
                        .to_cols_array_2d(),
                    self.camera.position.to_array(),
                ),
                0.0,
                -1.0,
                0,
                1,
                0,
            );

            let num_clusters = clusters.size() as usize;
            cluster_storage.truncate(num_clusters);

            if self.umbra_result == QueryErrorCode::Ok {
                occlusion_buffer = Some(ob);
                visible_cluster_bounds = Some(
                    cluster_storage
                        .into_iter()
                        .map(|i| {
                            let (min, max) = tome.get_cluster_bounds(i);
                            AxisAlignedBBox {
                                min: min.extend(1.0),
                                max: max.extend(1.0),
                            }
                            .expand(4.0)
                        })
                        .collect_vec(),
                );
            }
        }

        {
            let ext = self.renderer.externs.get_mut();
            ext.globals = self.scene_renderer.global_channels;

            let global_tex = &self.renderer.globals.textures;
            *ext.frame = externs::Frame {
                game_time: self.start_time.elapsed().as_secs_f32(),
                render_time: self.start_time.elapsed().as_secs_f32(),
                delta_game_time: delta_time,
                unk10: self.time_of_day / 3600.0,
                // exposure_time: 1.0 / 60.0,
                // exposure_scale: 2.0,          // view.settings().exposure_scale,
                // exposure_illum_relative: 1.0, // view.settings().exposure_illum_relative,
                specular_tint_lookup: global_tex.specular_tint_lookup.view.into(),
                specular_lobe_lookup: global_tex.specular_lobe_lookup.view.into(),
                specular_lobe_3d_lookup: global_tex.specular_lobe_3d_lookup.view.into(),
                iridescence_lookup: global_tex.iridescence_lookup.view.into(),
                ..*ext.frame.clone()
            };

            *ext.transparent = externs::Transparent {
                unk48: self.scene_renderer.main_view.shaded_read.view().into(),
                // unk00: view.atmosphere.sky_lookup_near.into(),
                // unk10: view.atmosphere.sky_lookup_far.into(),
                // unk00: todo!(), // t11, Atmosphere (near?)
                // unk08: todo!(), // t12, Atmosphere (3x2)
                // unk10: todo!(), // t13, Atmosphere (far?)
                // unk18: todo!(), // t14, 3d lightprobe
                // unk20: self.common.temporary_depth_angle_lookup.view.clone().into(), // t15
                // unk28: todo!(), // t16, 3d lightprobe
                // unk30: todo!(), // t17, 3d lightprobe
                // unk38: todo!(), // t18, 3d lightprobe
                // unk40: todo!(), // t19, 3d lightprobe
                // unk48: todo!(), // t20
                // unk50: todo!(), // t21
                // unk58: todo!(), // t22
                // unk60: todo!(), // t23
                // unk68: todo!(), // t24 (new in marathon)
                // unk70: todo!(), // t25 (new in marathon)
                unk80: vec4(0.22882, 0.00, 1.00, 45.00),
                unk90: vec4(0.00, 0.00, 1.17485, 2.86546),
                unka0: vec4(0.00, 0.00, 2.10913, 5.14044),
                unkb0: vec4(0.00, 0.00, 3.46762, 8.41667),
                unkc0: vec4(0.00, 0.00, 0.00, 0.00),

                // New in marathon
                unkd0: vec4(0.00, 0.00, 0.00, 0.00),
                unke0: vec4(0.00, 0.00, 0.00, 0.00),

                ..Default::default()
            };

            ext.cubemaps.vertex_ao = self.renderer.internal.white.view.into();

            self.camera.aspect_ratio = resolution.0 as f32 / resolution.1 as f32;
            self.controller.update_rotation(&mut self.camera);
            self.camera.update();
            ext.view.world_to_camera = self.camera.world_to_camera;
            ext.view.camera_to_projective = self.camera.camera_to_projective;
            self.scene_renderer.main_view.world_to_camera = self.camera.world_to_camera;
            self.scene_renderer.main_view.camera_to_projective = self.camera.camera_to_projective;
            ext.view.derive_matrices(resolution);
            self.scene_renderer.main_view.culling_frustum = self.camera.culling_frustum.clone();

            let near = Camera::NEAR;
            let far = Camera::FAR;
            ext.deferred.depth_constants = Vec4::new(
                1.0 / far,
                (far - near) / (far * near),
                0.00000000,
                0.00000000,
            );
            let view = &self.scene_renderer.main_view;
            ext.deferred.gbuffer_resolution_scale_offset = Vec4::new(
                view.resolution().0 as f32,
                view.resolution().1 as f32,
                0.0,
                0.0,
            );
            ext.deferred.deferred_depth = view.gbuffer.depth_read.depth_view().into();
            ext.deferred.deferred_rt0 = view.gbuffer.albedo.view().into();
            ext.deferred.deferred_rt1 = view.gbuffer.normal.view().into();
            ext.deferred.deferred_rt2 = view.gbuffer.rt3.view().into();
            ext.deferred.light_diffuse = view.light.light_diffuse.view().into();
            ext.deferred.light_specular = view.light.light_specular.view().into();
            ext.deferred.light_specular_ibl = view.light.light_specular_ibl.view().into();
            ext.deferred.sky_hemisphere = self.renderer.internal.default_hemisphere.view.into();

            let view = &self.scene_renderer.main_view;
            ext.decal.depth_read = view.gbuffer.depth_read.depth_view().into();
            ext.decal.normals_read = view.gbuffer.normal_read.view().into();
            ext.decal.depth_constants = ext.deferred.depth_constants;
            ext.decal.unk30 = vec4(
                resolution.0 as f32,
                resolution.1 as f32,
                1.0 / resolution.0 as f32,
                1.0 / resolution.1 as f32,
            );

            ext.water.unk00 = view.shaded_read.view().into();
            // ext.water.unk40 = Vec4::splat(fastrand::f32_inclusive());
            // ext.water.unk50 = Vec4::splat(fastrand::f32_inclusive());
            // ext.atmosphere.unk130 = Vec4::splat(fastrand::f32_inclusive());
            // ext.atmosphere.unk170 = fastrand::f32_inclusive();
            // ext.atmosphere.unk174 = fastrand::f32_inclusive();
            // ext.atmosphere.unk18c = fastrand::f32_inclusive();
            // ext.atmosphere.unk1a4 = fastrand::f32_inclusive();
            // ext.atmosphere.unk1a8 = fastrand::f32_inclusive();
            // ext.atmosphere.unk1ac = fastrand::f32_inclusive();
            // ext.atmosphere.unk188 = fastrand::f32_inclusive();
            // ext.shadow_mask.unk30 = fastrand::f32_inclusive();

            // ext.soft_deform.unk10 = Mat4::from_scale_rotation_translation(
            //     Vec3::splat(fastrand::f32_inclusive()),
            //     Quat::from_rotation_arc(
            //         Vec3::splat(fastrand::f32_inclusive()),
            //         Vec3::splat(fastrand::f32_inclusive()),
            //     ),
            //     Vec3::splat(fastrand::f32_inclusive()),
            // );

            *ext.global_lighting = externs::GlobalLighting {
                // unk08: self.renderer.gpu.placeholder_white.view.clone().into(),
                unk10: ext.get_global_channel_by_name("sun_color")
                    * ext.get_global_channel_by_name("sun_intensity").x,
                unk30: ext.get_global_channel_by_name("sun_light_direction"),
                unk50: ext.get_global_channel_by_name("sun_ambient_direction"),
                unk70: ext.get_global_channel_by_name("up_ambient_color")
                    * ext.get_global_channel_by_name("up_ambient_intensity").x,
                unk80: ext.get_global_channel_by_name("down_ambient_color")
                    * ext.get_global_channel_by_name("down_ambient_intensity").x,
                unk90: ext.get_global_channel_by_name("up_ambient_sharpness").x,
                unk94: ext.get_global_channel_by_name("down_ambient_sharpness").x,
                unka0: vec4(0.01, 0.01, -0.5, -0.5),
                unkb0: vec4(0.02, -2.0, 0.0, 0.0),
                // unkd0: vec4(f32::NAN, f32::NAN, 0.5, 0.5),
                unkc0: vec4(0.00333, -2.33333, 0.00, 0.00),
                ..Default::default()
            };

            let time = self.start_time.elapsed().as_secs_f32();
            self.renderer
                .globals
                .scopes
                .frame
                .write_initial_constants(
                    FrameScope {
                        game_time: ext.frame.game_time, //self.start_time.elapsed().as_secs_f32(),
                        render_time: ext.frame.render_time, //self.start_time.elapsed().as_secs_f32(),
                        delta_game_time: ext.frame.delta_game_time,
                        exposure_time: ext.frame.exposure_time,

                        // exposure_scale: 1.,
                        // exposure_illum_relative_glow: 1.,
                        // exposure_illum_relative: 1.,
                        // exposure_scale_for_shading: 1.,
                        exposure_scale: ext.frame.exposure_scale,
                        exposure_illum_relative_glow: ext.frame.exposure_illum_relative * 16.0,
                        exposure_scale_for_shading: ext.frame.exposure_scale,
                        exposure_illum_relative: ext.frame.exposure_illum_relative,

                        random_seed_scales: vec4(
                            time.mul_add(60.0, 33.75) * 1.258699,
                            time.mul_add(60.0, 60.0) * 0.9583125,
                            time.mul_add(60.0, 60.0) * 8.789123,
                            time.mul_add(60.0, 33.75) * 2.311535,
                        ),
                        unk3: vec4(0.5, 0.5, 0.0, 0.0),
                        unk4: vec4(1.0, 1.0, 0.0, 1.0),
                        unk5: vec4(0.00, -f32::NAN, 512.00, 0.00),
                        unk6: Vec4::ONE,
                    }
                    .to_array()
                    .as_ref(),
                )
                .expect("failed to copy frame constants");

            self.renderer
                .globals
                .scopes
                .transparent_advanced
                .write_initial_constants(&[
                    vec4(0.00227, 0.00896, 0.32782, 0.6419),
                    vec4(0.0026, 4.86115, 0.00198, 0.00002),
                    vec4(0.9158, 233.93063, 0.51102, 0.08905),
                    vec4(147.09909, 0.55492, 0.52397, 0.00),
                    vec4(0.00, 0.64794, 0.14063, 0.01563),
                    vec4(0.58584, 0.58584, 0.58584, 0.58584),
                    vec4(1.38137, 2.08133, 0.85451, 0.4165),
                    vec4(0.90933, 0.90933, 0.90933, 0.90933),
                    vec4(132.92885, 66.40444, 56.85342, 0.00),
                    vec4(132.92885, 66.40444, 1000.00, 0.0001),
                    vec4(131.92885, 65.40444, 55.85342, 0.67843),
                    vec4(131.92885, 65.40444, 999.00, 5.50),
                    vec4(0.00, 0.50, 25.57599, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.025, 10000.00, -9999.00, 1.00),
                    vec4(1.00, 1.00, 1.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(10.92799, 7.10136, 6.25467, 0.00),
                    vec4(0.00376, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00753, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.01759, 0.00),
                    vec4(-1.13485, 6.87303, -0.33715, 1.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(0.00, 0.00, 0.00, 0.00),
                    vec4(1.00, 0.00, 0.00, 0.00),
                ])
                .expect("Failed to write transparent_advanced initial constants");
        }

        let mut sun_dir = self
            .scene_renderer
            .get_global_channel_by_name("sun_light_direction")
            .unwrap_or_default()
            .xyz();
        if sun_dir.length() < 0.01 {
            sun_dir = Vec3::Z;
        }
        let sun_dir = -sun_dir.normalize();

        let sdsm_far_plane =
            visible_cluster_bounds
                .as_ref()
                .map_or(CascadeCalculator::MAX_FAR_PLANE, |c| {
                    let mut far_plane = 0f32;

                    for cluster in c {
                        far_plane = far_plane
                            .max(cluster.transformed(self.camera.world_to_camera).max.z.abs());
                    }

                    far_plane
                });

        for i in 0..CascadeCalculator::MAX_CASCADES {
            let view = &mut self.scene_renderer.main_view.shadow_views[i];
            let (near, far) = CascadeCalculator::get_depth_range(i, sdsm_far_plane)
                .expect("invalid cascade index for cascade calculation");

            let (world_to_camera, camera_to_projective) =
                self.camera.build_shadow_cascade(sun_dir, near, far);

            view.world_to_camera = world_to_camera;
            view.camera_to_projective = camera_to_projective;
            view.frustum =
                Frustum::from_view_and_projection(view.world_to_camera, view.camera_to_projective);
        }

        let vis = ViewVisibility {
            enabled: self.camera.projection != CameraProjection::Orthographic,
            culling_frustum: self.camera.culling_frustum.clone(),
            far_plane: Camera::FAR,
            position: self.camera.position,
            world_to_projective: self.camera.world_to_projective,
            occlusion_buffer,
            visible_cluster_bounds,
            render_stages: RenderStageSubscription::STANDARD_VIEW,
        };

        {
            let _scope = self.renderer.gpu.profiler_scope(&cmd, "visibility_test");
            s_visibility_test(&mut self.world, &vis);
        }

        self.scene_renderer.frame_packet.reset();

        {
            let _scope = self.renderer.gpu.profiler_scope(&cmd, "extract_frame");
            s_extract_frame_packet(
                &self.world,
                &mut self.scene_renderer,
                &vis,
                self.subscribed_features,
                self.draw_sun_shadows,
            );
        }

        self.scene_renderer.render(
            &mut cmd,
            &vis,
            self.render_mode.into(),
            self.draw_sun_shadows,
        );
    }

    fn show_toolbar(&mut self, ui: &mut Ui) {
        ui.style_mut().spacing.item_spacing = vec2(8.0, 0.0);
        egui::containers::menu::MenuButton::new(GoogleMaterialSymbols::Tune.to_string())
            .config(
                MenuConfig::new().close_behavior(if self.keep_settings_open {
                    egui::PopupCloseBehavior::IgnoreClicks
                } else {
                    egui::PopupCloseBehavior::CloseOnClickOutside
                }),
            )
            .ui(ui, |ui| {
                self.show_settings_ui(ui);
            })
            .0
            .on_hover_text("Scene Settings");

        // if ui
        //     .selectable_label(
        //         false, // self.show_surface_viewer,
        //         GoogleMaterialSymbols::ImageSearch.to_string(),
        //     )
        //     .clicked()
        // {
        //     // self.show_surface_viewer = !self.show_surface_viewer;
        // }

        if ui
            .selectable_label(
                self.show_channel_editor,
                GoogleMaterialSymbols::BarChart4Bars.to_string(),
            )
            .clicked()
        {
            self.show_channel_editor = !self.show_channel_editor;
        }

        self.render_mode.ui(ui);
        self.subscribed_features.show_input(ui);
    }

    fn show_settings_ui(&mut self, ui: &mut Ui) {
        ui.label(format!("Camera Pos: {:.1?}", self.camera.position));
        ui.label(format!(
            "Camera Yaw/Pitch: {:.1}/{:.1}",
            self.controller.yaw_pitch().x,
            self.controller.yaw_pitch().y
        ));
        // ui.label(format!("Camera Forward: {:.3?}", self.camera.forward()));

        let (bearing, dir) = compass_heading(self.camera.forward());
        ui.label(format!("Heading {} ({:.1}°)", dir, bearing));

        ui.style_mut()
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(16.0));
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(24.0));
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(16.0));

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            ui.heading("Scene Settings");
            if ui
                .selectable_label(
                    self.keep_settings_open,
                    GoogleMaterialSymbols::PushPin.to_string(),
                )
                .on_hover_text("Keep panel open")
                .clicked()
            {
                self.keep_settings_open = !self.keep_settings_open;
            }
            // #[cfg(debug_assertions)]
            // if ui
            //     .selectable_label(false, GoogleMaterialSymbols::Code.to_string())
            //     .on_hover_text("Load camera cbuffers")
            //     .clicked()
            // {
            //     self.load_camera_cbuffers();
            // }
            // #[cfg(debug_assertions)]
            // if ui
            //     .selectable_label(
            //         self.lock_resolution,
            //         GoogleMaterialSymbols::ScreenLockLandscape.to_string(),
            //     )
            //     .on_hover_text("Lock resolution to 1920x1080")
            //     .clicked()
            // {
            //     self.lock_resolution = !self.lock_resolution;
            // }
        });

        ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = vec2(4.0, 1.0);
            ui.spacing_mut().interact_size = egui::vec2(100.0, 32.0);
            let ext = self.renderer.externs.get_mut();
            egui::DragValue::new(&mut ext.decal.unk20.x)
                .fixed_decimals(4)
                .speed(0.01)
                .ui(ui);
            egui::DragValue::new(&mut ext.decal.unk20.y)
                .fixed_decimals(4)
                .speed(0.01)
                .ui(ui);
        });

        // ui.checkbox(&mut view_settings.autoexposure, "Auto-exposure")
        //     .setting_description_tooltip(
        //         "Enables automatic exposure adjustment based on scene brightness.",
        //         PerformanceImpact::None,
        //     );

        // if settings_mut.autoexposure {
        //     ui.strong("Target Luminance");
        //     ui.spacing_mut().slider_width = ui.available_width() * 0.75;
        //     egui::Slider::new(
        //         &mut self.view.autoexposure.config.target_luminance,
        //         0.000002..=0.04,
        //     )
        //     .logarithmic(false)
        //     .show_value(true)
        //     .ui(ui);
        // }

        ui.strong("Exposure Scale");
        ui.spacing_mut().slider_width = ui.available_width() * 0.75;
        let ext = self.renderer.externs.get_mut();
        egui::Slider::new(&mut ext.frame.exposure_scale, 0.001..=4.0)
            .logarithmic(true)
            .show_value(true)
            .ui(ui);

        ui.strong("Exposure Illum Relative");
        ui.spacing_mut().slider_width = ui.available_width() * 0.75;
        egui::Slider::new(&mut ext.frame.exposure_illum_relative, 0.01..=2.0)
            .logarithmic(false)
            .show_value(true)
            .ui(ui);

        ui.add_space(4.0);

        // Time of Day slider
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            ui.strong("Time of Day");
            ui.label(format!(
                "({:02}:{:02})",
                (self.time_of_day / 3600.0 * 24.0).floor() as u32,
                ((self.time_of_day / 3600.0 * 24.0 * 60.0) % 60.0).floor() as u32
            ));
        });

        ui.spacing_mut().slider_width = ui.available_width();

        const DAYNIGHT_GRADIENT: ImageSource<'static> =
            egui::include_image!("../../../assets/ui/daynight_gradient_bar.png");
        Image::new(DAYNIGHT_GRADIENT).paint_at(
            ui,
            Rect::from_min_size(
                ui.cursor().min + vec2(0.0, 6.0),
                Vec2::new(ui.available_width(), 8.0),
            ),
        );

        ui.scope(|ui| {
            ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::from_black_alpha(48);
            ui.style_mut().visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(8.0_f32, egui::Color32::WHITE);

            egui::Slider::new(&mut self.time_of_day, 0.0..=3600.0)
                .show_value(false)
                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 })
                .ui(ui);
        });

        ui.checkbox(&mut self.animate_time_of_day, "Automate Time")
            .on_hover_text("Automatically animate time of day");

        // Raininess slider
        ui.strong("Raininess");
        const RAININESS_GRADIENT: ImageSource<'static> =
            egui::include_image!("../../../assets/ui/raininess_gradient_bar.png");
        Image::new(RAININESS_GRADIENT).paint_at(
            ui,
            Rect::from_min_size(
                ui.cursor().min + vec2(0.0, 6.0),
                Vec2::new(ui.available_width(), 8.0),
            ),
        );

        ui.scope(|ui| {
            ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::from_black_alpha(48);
            ui.style_mut().visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(8.0_f32, egui::Color32::WHITE);

            egui::Slider::new(&mut self.raininess, 0.0..=1.0)
                .show_value(false)
                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 })
                .ui(ui);
        });

        // Heat cascade slider
        ui.strong("Heat Cascade");
        const HEAT_CASCADE_GRADIENT: ImageSource<'static> =
            egui::include_image!("../../../assets/ui/heat_cascade_gradient_bar.png");
        Image::new(HEAT_CASCADE_GRADIENT).paint_at(
            ui,
            Rect::from_min_size(
                ui.cursor().min + vec2(0.0, 6.0),
                Vec2::new(ui.available_width(), 8.0),
            ),
        );

        ui.scope(|ui| {
            ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::from_black_alpha(48);
            ui.style_mut().visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(8.0_f32, egui::Color32::WHITE);

            egui::Slider::new(&mut self.heat_cascade, 0.0..=1.0)
                .show_value(false)
                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 })
                .ui(ui);
        });

        ui.spacing_mut().slider_width = ui.available_width() * 0.75;
        egui::Slider::new(&mut self.camera.fov_y, 10.0..=120.0)
            .text("Camera FOV")
            .show_value(true)
            .ui(ui);

        ui.spacing_mut().slider_width = 256.0;
        let mut resolution_scale = self.scene_renderer.main_view.resolution_scale();
        if ui
            .add(
                egui::Slider::new(&mut resolution_scale, 0.25..=2.0)
                    .step_by(0.25)
                    .text("Resolution Scale")
                    .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
            )
            .changed()
        {
            self.scene_renderer
                .main_view
                .set_resolution_scale(resolution_scale);
        }

        ui.separator();

        // ui.checkbox(&mut view_settings.vertex_ao, "Vertex AO")
        //     .setting_description_tooltip(
        //         "Enables ambient occlusion based on mesh vertex data.\nCan highly impact the look \
        //              and feel of a scene, as it darkens indoor areas and crevices.",
        //         PerformanceImpact::None,
        //     );

        // ui.checkbox(&mut view_settings.bloom, "Bloom")
        //     .setting_description_tooltip(
        //         "Enables bloom effect, which adds a glow to bright areas of the scene.",
        //         PerformanceImpact::Low,
        //     );

        // ui.checkbox(&mut view_settings.volumetrics, "Volumetrics")
        //     .setting_description_tooltip(
        //         "Enables volumetric lighting effects, such as light shafts and fog.",
        //         PerformanceImpact::Medium,
        //     );
        // ui.checkbox(&mut view_settings.shadows, "Local Shadows")
        //     .setting_description_tooltip(
        //         "Enables (static) shadows for local lights.",
        //         PerformanceImpact::Medium,
        //     );

        let mut decorator_quality = DECORATOR_QUALITY.load(Ordering::Relaxed) as usize;
        egui::Slider::new(&mut decorator_quality, 0..=2)
            .text("Decorator Quality").custom_formatter(|n, _| match n as u8 {
                0 => "High".to_string(),
                1 => "Medium".to_string(),
                2 => "Low".to_string(),
                _ => n.to_string(),
            })
            .show_value(true).ui(ui)
            .setting_description_tooltip(
                "Sets the quality level for decorator rendering, which affects the visual fidelity of \
                 small details like grass and debris. Higher quality levels may significantly \
                 reduce performance on lower-end hardware.",
                PerformanceImpact::High,
            );
        DECORATOR_QUALITY.store(decorator_quality as u8, Ordering::Relaxed);

        ui.checkbox(&mut self.draw_sun_shadows, "Sun Shadows")
            .setting_description_tooltip(
                "Enables shadows for the sun light.",
                PerformanceImpact::High,
            );

        ui.checkbox(&mut self.umbra_enabled, "Umbra Culling")
            .setting_description_tooltip(
                "Enables Umbra3 occlusion culling for geometry.",
                PerformanceImpact::High,
            );

        // ui.checkbox(&mut view_settings.anti_aliasing, "Anti-Aliasing")
        //     .setting_description_tooltip(
        //         "Enables FXAA anti-aliasing to smooth out jagged edges.",
        //         PerformanceImpact::Low,
        //     );

        // ui.collapsing("Advanced", |ui| {
        //         ui.checkbox(&mut view_settings.multithreading, "Multi-threaded Submit")
        //             .setting_description_tooltip(
        //                 "Enables multi-threaded submission of commands to the GPU. May improve \
        //                  performance on systems with many CPU cores, but can introduce stuttering on \
        //                  older systems",
        //                 PerformanceImpact::High,
        //             );

        //         ui.checkbox(&mut view_settings.hzb_culling, "HZB Culling")
        //             .setting_description_tooltip(
        //                 "Enables Hierarchical Z-Buffer (HZB) culling to optimize rendering by \
        //                  discarding occluded objects.",
        //                 PerformanceImpact::High,
        //             );
        //     });
    }

    fn show_channel_editor(&mut self, ui: &mut Ui) {
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(16.0));
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(16.0));

        ui.checkbox(&mut self.only_show_used_channels, "Only show used channels");

        // let automated_ids = s_get_all_global_channel_ids(&self.world);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Global Channels");
            for (i, channel) in self.scene_renderer.global_channels.iter_mut().enumerate() {
                let frequency =
                    self.renderer.externs.global_channel_frequency[i].load(Ordering::Relaxed);

                if self.only_show_used_channels && frequency == 0 {
                    continue;
                }

                let Some(channel_id) = self.renderer.externs.global_ids.get(i) else {
                    continue;
                };
                // let is_automated = automated_ids.contains(channel_id);
                let is_automated = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if let Some(name) = get_global_channel_name(*channel_id) {
                        ui.label(format!("Channel #{i} {name} (0x{channel_id:08X})"));
                    } else {
                        ui.label(format!("Channel #{i} 0x{channel_id:08X}"));
                    }
                    if is_automated {
                        ui.weak("(automated)");
                    }
                    ui.weak(format!("used x{frequency}"))
                });
                ui.add_enabled_ui(!is_automated, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().button_padding = vec2(4.0, 1.0);
                        ui.spacing_mut().interact_size = egui::vec2(100.0, 32.0);
                        egui::DragValue::new(&mut channel.x)
                            .fixed_decimals(4)
                            .speed(0.01)
                            .ui(ui);
                        egui::DragValue::new(&mut channel.y)
                            .fixed_decimals(4)
                            .speed(0.01)
                            .ui(ui);
                        egui::DragValue::new(&mut channel.z)
                            .fixed_decimals(4)
                            .speed(0.01)
                            .ui(ui);
                        egui::DragValue::new(&mut channel.w)
                            .fixed_decimals(4)
                            .speed(0.01)
                            .ui(ui);
                    });
                });
            }
        });
    }

    pub const fn output_srv(&self) -> vk::ImageView {
        self.scene_renderer.main_view.output.view()
    }

    pub fn render_to_texture(&mut self, resolution: (u32, u32)) -> anyhow::Result<RenderTarget> {
        self.render(1.0 / 60.0, resolution);

        self.scene_renderer.main_view.output.take()
    }

    pub const fn focus_on(&mut self, position: Vec3) {
        match &mut self.controller {
            CameraController::Orbit { target, .. } => {
                *target = position;
            }
            CameraController::FirstPerson { .. } => {
                self.camera.position = position;
            }
        }
    }

    pub fn focus_fit_ortho(&mut self, aabb: &AxisAlignedBBox) {
        match &mut self.controller {
            CameraController::Orbit { target, .. } => {
                *target = aabb.centroid();
                self.camera.max_ortho_width = aabb.extents().length() * 0.75;
            }
            CameraController::FirstPerson { .. } => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Lookdev,
    Shaded,
    ShadedNoSun,
    ShadingOnly,
    // Matcap,

    // Material:
    Albedo,
    Smoothness,
    Metalness,
    AmbientOcclusion,
    Emission,
    EmissionIntensity,
    Transmission,
    IridescenceId,

    // Geometry:
    DepthEdges,
    WorldNormal,
    Overdraw,

    // Lighting:
    LightDiffuse,
    LightSpecular,
}

impl RenderMode {
    /// Returns true if the render mode UI changed the value
    pub fn ui(&mut self, ui: &mut Ui) -> bool {
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(16.0));

        let mut changed = false;
        egui::ComboBox::from_id_salt("Render Mode")
            .height(400.0)
            .selected_text(format!("{} {:?}", GoogleMaterialSymbols::EvShadow, self))
            .show_ui(ui, |ui| {
                ui.style_mut()
                    .text_styles
                    .insert(TextStyle::Button, FontId::proportional(16.0));
                ui.style_mut().spacing.button_padding = Vec2::new(8.0, 2.0);
                ui.style_mut().spacing.item_spacing = Vec2::ZERO;

                macro_rules! mode {
                    ($ui:ident, $variant:expr, $name:literal) => {
                        if $ui.selectable_label(*self == $variant, $name).clicked() {
                            *self = $variant;
                            changed = true;
                        }
                    };
                }

                mode!(ui, Self::Lookdev, "Lookdev");
                mode!(ui, Self::Shaded, "Shaded");
                mode!(ui, Self::ShadedNoSun, "Shaded (No sun)");
                mode!(
                    ui,
                    Self::ShadingOnly,
                    "Shading Only (No atmosphere, no sun)"
                );
                // mode!(ui, Self::Matcap, "Matcap");

                ui.section_separator("Material:");
                mode!(ui, Self::Albedo, "Albedo");
                mode!(ui, Self::Smoothness, "Smoothness");
                mode!(ui, Self::Metalness, "Metalness");
                mode!(ui, Self::AmbientOcclusion, "Ambient Occlusion");
                mode!(ui, Self::Emission, "Emission");
                mode!(ui, Self::EmissionIntensity, "Emission Intensity");
                mode!(ui, Self::Transmission, "Transmission");
                mode!(ui, Self::IridescenceId, "Iridescence ID");

                ui.section_separator("Geometry:");
                mode!(ui, Self::DepthEdges, "Depth Edges");
                mode!(ui, Self::WorldNormal, "World Normal");
                mode!(ui, Self::Overdraw, "Overdraw");

                ui.section_separator("Lighting:");
                mode!(ui, Self::LightDiffuse, "Diffuse Light");
                mode!(ui, Self::LightSpecular, "Specular Light");
            });

        changed
    }
}

impl From<RenderMode> for Option<DebugPipeline> {
    fn from(val: RenderMode) -> Self {
        match val {
            RenderMode::Lookdev => Some(DebugPipeline::LookDev),
            RenderMode::Shaded => Some(DebugPipeline::GlobalLightingShading),
            RenderMode::ShadedNoSun => Some(DebugPipeline::DeferredShading),
            RenderMode::ShadingOnly => Some(DebugPipeline::DeferredShadingNoAtm),
            // RenderMode::Matcap => Some(DebugPipeline::Matcap),
            RenderMode::Albedo => Some(DebugPipeline::Albedo),
            RenderMode::Smoothness => Some(DebugPipeline::Smoothness),
            RenderMode::Metalness => Some(DebugPipeline::Metalness),
            RenderMode::AmbientOcclusion => Some(DebugPipeline::AmbientOcclusion),
            RenderMode::Emission => Some(DebugPipeline::Emission),
            RenderMode::EmissionIntensity => Some(DebugPipeline::EmissionIntensity),
            RenderMode::Transmission => Some(DebugPipeline::Transmission),
            RenderMode::IridescenceId => Some(DebugPipeline::Overcoat),
            RenderMode::DepthEdges => Some(DebugPipeline::DepthEdges),
            RenderMode::WorldNormal => Some(DebugPipeline::WorldNormal),
            RenderMode::Overdraw => Some(DebugPipeline::Overdraw),
            RenderMode::LightDiffuse => Some(DebugPipeline::LightDiffuse),
            RenderMode::LightSpecular => Some(DebugPipeline::LightSpecular),
        }
    }
}

impl ExternalDataWidgetExt for FeatureRendererSubscription {
    fn show_input(&mut self, ui: &mut Ui) -> egui::Response {
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(16.0));

        egui::ComboBox::from_id_salt("Feature Renderers")
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .height(400.0)
            .selected_text(format!(
                "{} Enabled Features",
                GoogleMaterialSymbols::CheckBox
            ))
            .show_ui(ui, |ui| {
                ui.style_mut()
                    .text_styles
                    .insert(TextStyle::Button, FontId::proportional(16.0));
                ui.style_mut().spacing.button_padding = Vec2::new(8.0, 2.0);
                ui.style_mut().spacing.item_spacing = Vec2::ZERO;

                let ctrl = ui.input(|i| i.modifiers.ctrl);
                let alt = ui.input(|i| i.modifiers.alt);
                macro_rules! feature {
                    ($ui:ident, $flag:expr, $name:literal) => {
                        if $ui.selectable_label(self.contains($flag), $name).clicked() {
                            if ctrl {
                                *self = Self::empty();
                                self.insert($flag);
                            } else if alt {
                                *self = Self::all();
                                self.remove($flag);
                            } else if self.contains($flag) {
                                self.remove($flag);
                            } else {
                                self.insert($flag);
                            }
                        }
                    };
                }

                feature!(ui, Self::CHUNKED_INSTANCE_OBJECTS, "Static Objects");
                feature!(ui, Self::TERRAIN_PATCH, "Terrain Patches");
                feature!(ui, Self::RIGID_OBJECT, "Rigid Objects");
                feature!(ui, Self::SKY_TRANSPARENT, "Sky Transparents");
                feature!(ui, Self::SPEEDTREE_TREES, "Decorators");
                feature!(ui, Self::DYNAMIC_DECALS, "Dynamic Decals");
                feature!(ui, Self::ROAD_DECALS, "Road Decals");
                feature!(ui, Self::WATER, "Water");
                ui.add_enabled_ui(false, |ui| {
                    feature!(ui, Self::LENS_FLARES, "Lens Flares");
                    feature!(ui, Self::PARTICLES, "Particles");
                });

                ui.section_separator("Lighting");
                feature!(ui, Self::CUBEMAPS, "Cubemaps");
                feature!(ui, Self::CHUNKED_LIGHTS, "Chunked Lights");
                feature!(ui, Self::DEFERRED_LIGHTS, "Deferred Lights");
            })
            .response
    }
}

fn compass_heading(forward: Vec3) -> (f32, &'static str) {
    const DIRS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

    let bearing_deg = (-forward.x)
        .atan2(-forward.y)
        .to_degrees()
        .rem_euclid(360.0); // map [-180, 180] to [0, 360]

    let index = ((bearing_deg + 22.5) / 45.0) as usize % 8;
    (bearing_deg, DIRS[index])
}

trait SettingDescriptionTooltipExt {
    fn setting_description_tooltip(
        self,
        description: &str,
        performance_impact: PerformanceImpact,
    ) -> Self;
}

impl SettingDescriptionTooltipExt for Response {
    fn setting_description_tooltip(
        self,
        description: &str,
        performance_impact: PerformanceImpact,
    ) -> Self {
        self.on_hover_ui(|ui| {
            ui.style_mut()
                .text_styles
                .insert(TextStyle::Body, FontId::proportional(16.0));

            let perf_color = match performance_impact {
                PerformanceImpact::None => egui::Color32::GRAY,
                PerformanceImpact::Low => egui::Color32::GREEN,
                PerformanceImpact::Medium => egui::Color32::YELLOW,
                PerformanceImpact::High => egui::Color32::RED,
            };

            ui.label(description);
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(0.0);
                ui.label("Performance Impact: ");
                ui.label(
                    RichText::new(format!("{:?}", performance_impact))
                        .color(perf_color)
                        .strong(),
                );
            });
        })
    }
}
#[derive(Debug)]
enum PerformanceImpact {
    None,
    Low,
    Medium,
    High,
}
