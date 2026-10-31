use std::sync::Arc;

use deimos_ecs::world::pattern::spawn_pattern;
use deimos_render::{camera::Camera, renderer::Renderer};
use egui::{Color32, Rect, Vec2, vec2};
use glam::Vec3;
use google_material_symbols::GoogleMaterialSymbols;
use tiger_pkg::TagHash;

use crate::{
    app::SharedState,
    task::Task,
    ui::{
        GuiRenderer,
        scene::{Scene, controller::CameraController},
        util::UiExt,
    },
    world::pattern::load_component,
};

pub struct ModelViewTab {
    pub tag: TagHash,
    load_task: Task<hecs::World>,
    scene: Box<Scene>,

    errored: bool,
}

impl ModelViewTab {
    pub fn new_pattern(
        renderer: &Arc<Renderer>,
        shared: &Arc<SharedState>,
        tag: TagHash,
    ) -> anyhow::Result<Self> {
        let renderer_clone = renderer.clone();

        Ok(Self {
            load_task: Task::new(format!("load_pattern({tag})"), move || {
                let mut world = hecs::World::new();
                spawn_pattern(
                    &mut world,
                    tag,
                    None,
                    None,
                    |world, entity, pattern, data, component| {
                        load_component(&renderer_clone, world, entity, pattern, data, component)
                    },
                )
                .expect("Failed to load map into world");
                world
            }),
            tag,
            scene: Box::new(
                Scene::new(renderer, &shared, Camera::default())?
                    .with_controller(CameraController::new_orbit(Vec3::ZERO, 2.5)),
            ),
            errored: false,
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, gui_renderer: &mut GuiRenderer) {
        if let Some(model) = self.load_task.get() {
            match model {
                Ok(world) => {
                    self.scene.set_world(world);
                    info!("Model loaded successfully");
                }
                Err(_e) => {
                    error!("Failed to load model: unknown error");
                    self.errored = true;
                }
            }
        }

        if self.load_task.is_pending() {
            let (_, rect) = ui.allocate_space(ui.available_size());
            ui.painter()
                .rect_filled(rect, 0, Color32::from_rgb(14, 24, 28));
            ui.d_paint_spinner_at(Rect::from_center_size(rect.center(), Vec2::splat(96.0)));
            ui.painter().text(
                rect.center() + vec2(0.0, 42.0),
                egui::Align2::CENTER_TOP,
                "Loading...",
                egui::FontId::proportional(24.0),
                Color32::GRAY,
            );
        } else if self.errored {
            let (_, rect) = ui.allocate_space(ui.available_size_before_wrap());
            ui.painter()
                .rect_filled(rect, 0, Color32::from_rgb(28, 14, 14));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                GoogleMaterialSymbols::Error,
                egui::FontId::proportional(96.0),
                Color32::DARK_RED,
            );
            ui.painter().text(
                rect.center() + vec2(0.0, 48.0),
                egui::Align2::CENTER_TOP,
                "Model load failed",
                egui::FontId::proportional(24.0),
                Color32::DARK_RED,
            );
            ui.painter().text(
                rect.center() + vec2(0.0, 82.0),
                egui::Align2::CENTER_TOP,
                "See logs for error information",
                egui::FontId::proportional(16.0),
                Color32::DARK_RED,
            );
        } else {
            self.scene.show(ui, gui_renderer, ui.available_size());
        }
    }
}
