use std::{
    sync::{Arc, LazyLock},
    time::Instant,
};

use anyhow::Context;
use deimos_data::activity::SActivity;
use deimos_ecs::world::map::{
    load_activity_for_map_into_world, load_activity_phase_into_world, load_map_into_world,
};
use deimos_render::{camera::Camera, renderer::Renderer};
use egui::{Color32, Rect, Vec2, vec2};
use google_material_symbols::GoogleMaterialSymbols;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    app::SharedState,
    task::Task,
    ui::{
        GuiRenderer,
        scene::{Scene, controller::CameraController},
        util::{DButton, UiExt},
    },
    world::pattern::load_component,
};

pub struct ActivityTab {
    renderer: Arc<Renderer>,
    _state: Arc<SharedState>,
    _activity: Arc<SActivity>,
    maps: Vec<ActivityMap>,
    current_map_index: usize,

    pub tag: TagHash,
    pub name: String,
    scene: Box<Scene>,
}

impl ActivityTab {
    pub fn new(state: &Arc<SharedState>, tag: TagHash, name: String) -> anyhow::Result<Self> {
        let activity: SActivity = package_manager()
            .read_tag_struct(tag)
            .context("Failed to read SActivity")?;
        let activity = Arc::new(activity);

        let mut maps = vec![];
        for map in &activity.unk50 {
            maps.push(ActivityMap {
                activity: activity.clone(),
                index: maps.len(),
                name: state.get_string_by_package(
                    &package_manager().package_paths[&tag.pkg_id()].name,
                    map.bubble_name,
                ),
                load_task: Task::default(),
                world: None,
                state: ActivityLoadState::Unloaded,
            });
        }
        if let Some(map) = maps.first_mut() {
            map.start_load(state.renderer.clone());
        }

        Ok(Self {
            renderer: state.renderer.clone(),
            _state: state.clone(),
            _activity: activity,
            current_map_index: 0,
            maps,
            tag,
            name,
            scene: Box::new(
                Scene::new(&state.renderer.clone(), state, Camera::default())?
                    .with_controller(CameraController::new_first_person()),
            ),
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, gui_renderer: &mut GuiRenderer) {
        if self.maps.len() > 1 {
            egui::Panel::left(format!("activity_{}_map_list", self.tag)).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([true, false])
                    .id_salt("map_list_packages")
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        for (i, map) in self.maps.iter_mut().enumerate() {
                            let state = map.poll_load();

                            let mut load_sym = state.symbol();
                            if self.current_map_index == i
                                && load_sym as u32 == GoogleMaterialSymbols::CheckCircle as u32
                            {
                                load_sym = GoogleMaterialSymbols::RadioButtonChecked;
                            }

                            let btn = if self.current_map_index == i {
                                DButton::new_black((load_sym.to_string(), map.name.clone()))
                            } else {
                                DButton::new((load_sym.to_string(), map.name.clone()))
                            };

                            if btn.min_size(vec2(384.0, 32.0)).ui(ui).clicked() {
                                self.current_map_index = i;
                                map.start_load(self.renderer.clone());
                            }
                        }
                    });
            });
        }

        let Some(map) = self.maps.get_mut(self.current_map_index) else {
            return;
        };

        match map.poll_load() {
            ActivityLoadState::Unloaded => {}
            ActivityLoadState::Loading => {
                let (_, rect) = ui.allocate_space(ui.available_size_before_wrap());
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
            }
            ActivityLoadState::Error => {
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
                    "Map load failed",
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
            }
            ActivityLoadState::Loaded => {
                if let Some(world) = map.world.as_mut() {
                    std::mem::swap(world, &mut self.scene.world);
                    self.scene.show(ui, gui_renderer, ui.available_size());
                    std::mem::swap(&mut self.scene.world, world);
                }
            }
        }
    }
}

struct ActivityMap {
    activity: Arc<SActivity>,
    index: usize,

    name: String,
    load_task: Task<hecs::World>,
    world: Option<hecs::World>,
    state: ActivityLoadState,
}

impl ActivityMap {
    fn start_load(&mut self, renderer: Arc<Renderer>) {
        if matches!(self.state, ActivityLoadState::Unloaded) {
            self.state = ActivityLoadState::Loading;
            let activity = self.activity.clone();
            let map_index = self.index;
            self.load_task = Task::new(format!("load_activity_map({map_index})"), move || {
                let mut world = hecs::World::new();
                let activity_map = &activity.unk50[map_index];
                // TODO(cohae): It's possible to have multiple maps per phase (see Tower), how do we handle showing those?
                let map_hash = activity_map.map_references[0];

                load_map_into_world(
                    map_hash.hash32(),
                    &mut world,
                    |world, entity, pattern, data, component| {
                        load_component(&renderer, world, entity, pattern, data, component)
                    },
                )
                .expect("Failed to load map");

                if activity.ambient_activity.is_some()
                    && let Err(e) = load_activity_for_map_into_world(
                        activity.ambient_activity,
                        activity_map.bubble_name,
                        &mut world,
                        |world, entity, pattern, data, component| {
                            load_component(&renderer, world, entity, pattern, data, component)
                        },
                    )
                {
                    error!("Failed to load ambient activity: {e}");
                }

                for unk in &activity_map.unk18 {
                    if let Err(e) = load_activity_phase_into_world(
                        unk,
                        &mut world,
                        |world, entity, pattern, data, component| {
                            load_component(&renderer, world, entity, pattern, data, component)
                        },
                    ) {
                        error!(
                            "Activity phase load for {} failed: {e}",
                            unk.unk_entity_reference.taghash()
                        );
                    }
                }
                world
            });
        }
    }

    #[must_use]
    fn poll_load(&mut self) -> &ActivityLoadState {
        if let Some(res) = self.load_task.get() {
            match res {
                Ok(world) => {
                    self.state = ActivityLoadState::Loaded;
                    self.world = Some(world);
                }
                Err(_err) => {
                    // TODO(cohae): Proper error handling and user popup(?)
                    error!("Activity map load failed");
                    self.state = ActivityLoadState::Error;
                }
            }
        }

        &self.state
    }
}

enum ActivityLoadState {
    Unloaded,
    Loading,
    Loaded,
    Error,
}

impl ActivityLoadState {
    fn symbol(&self) -> GoogleMaterialSymbols {
        match self {
            Self::Unloaded => GoogleMaterialSymbols::Circle,
            Self::Loading => {
                static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);
                let elapsed = Instant::now().duration_since(*START_TIME);
                const SYMBOLS: [GoogleMaterialSymbols; 5] = [
                    GoogleMaterialSymbols::Circle,
                    GoogleMaterialSymbols::ClockLoader20,
                    GoogleMaterialSymbols::ClockLoader40,
                    GoogleMaterialSymbols::ClockLoader60,
                    GoogleMaterialSymbols::ClockLoader80,
                ];

                SYMBOLS[(elapsed.as_millis() / 100) as usize % SYMBOLS.len()]
            }
            Self::Loaded => GoogleMaterialSymbols::CheckCircle,
            Self::Error => GoogleMaterialSymbols::Error,
        }
    }
}
