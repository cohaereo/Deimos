pub mod activity;
pub mod activity_list;
pub mod entity_list;
pub mod home;
pub mod map;
pub mod map_list;
pub mod model_list;
pub mod model_view;
pub mod settings;
pub mod static_list;

// pub mod tag_lookup;
// pub mod test_scene;

use std::{fmt::Display, sync::Arc};

use egui::Margin;
use egui_dock::{DockState, TabPath};
use google_material_symbols::GoogleMaterialSymbols;
use home::HomeTab;

use crate::ui::{
    GuiRenderer,
    tabs::{
        activity::ActivityTab, activity_list::ActivityListTab, entity_list::EntityListTab,
        map::MapTab, map_list::MapListTab, model_view::ModelViewTab, settings::SettingsTab,
        static_list::StaticListTab,
    },
};

pub enum Tab {
    Home,
    Settings,
    EntityList(Box<EntityListTab>),
    ModelView(Box<ModelViewTab>),
    StaticList(Box<StaticListTab>),
    MapList(MapListTab),
    Map(MapTab),
    ActivityList(ActivityListTab),
    Activity(ActivityTab),
    // TestScene(TestSceneTab),
    // TagLookup(TagLookupTab),
}

impl Tab {
    pub const fn is_fixed(&self) -> bool {
        matches!(self, Self::Home | Self::Settings)
    }

    /// Returns an arbitrary key that's unique for the corresponding tab type. Tabs with only 1 instance return 0
    pub const fn key(&self) -> u64 {
        match self {
            Self::Home
            | Self::Settings
            | Self::MapList(_)
            | Self::ActivityList(_)
            | Self::EntityList(_)
            | Self::StaticList(_) => 0,
            Self::ModelView(tab) => tab.tag.0 as u64,
            Self::Map(tab) => tab.tag.0 as u64,
            Self::Activity(tab) => tab.tag.0 as u64,
            // Tab::TestScene(_) => 0,
            // Tab::TagLookup(_) => 0,
        }
    }
}

impl Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Settings => GoogleMaterialSymbols::Settings.to_string(),
            Self::Home => format!("{} Home", GoogleMaterialSymbols::Home),
            Self::EntityList(_) => format!("{} Entities", GoogleMaterialSymbols::ChessPawn),
            Self::ModelView(tab) => {
                format!("{} Model {}", GoogleMaterialSymbols::_3dRotation, tab.tag)
            }
            Self::StaticList(_) => format!("{} Statics", GoogleMaterialSymbols::Landscape),
            Self::MapList(_) => format!("{} Maps", GoogleMaterialSymbols::Map),
            Self::Map(tab) => format!("{} ({})", tab.name, tab.tag),
            Self::ActivityList(_) => {
                format!("{} Activities", GoogleMaterialSymbols::StadiaController)
            }
            Self::Activity(tab) => format!("{} ({})", tab.name, tab.tag),
            // Tab::TestScene(_) => format!("{} Test Scene", GoogleMaterialSymbols::Experiment),
            // Tab::TagLookup(_) => format!("{} Tag Lookup", GoogleMaterialSymbols::Search),
        };

        f.write_str(&s)
    }
}

pub struct TabViewer<'a> {
    pub added_nodes: &'a mut Vec<Tab>,
    pub renderer: &'a mut GuiRenderer,
    pub shared_state: &'a Arc<crate::app::SharedState>,
}

impl<'a> egui_dock::TabViewer for TabViewer<'a> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(format!("{}_{}", tab.key(), self.title(tab).text()))
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        egui::Frame::new()
            .outer_margin(if tab.is_fixed() {
                Margin::symmetric(127, 64)
            } else {
                Margin::ZERO
            })
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| match tab {
                    Tab::Home => {
                        self.process_result(HomeTab.ui(ui, self.shared_state));
                    }
                    Tab::Settings => {
                        SettingsTab::ui(ui, self.shared_state);
                    }
                    Tab::EntityList(tab) => {
                        let res = tab.ui(ui, self.renderer);
                        self.process_result(res);
                    }
                    Tab::ModelView(tab) => {
                        tab.ui(ui, self.renderer);
                    }
                    Tab::StaticList(tab) => {
                        let res = tab.ui(ui, self.renderer);
                        self.process_result(res);
                    }
                    Tab::MapList(tab) => {
                        self.process_result(tab.ui(ui));
                    }
                    Tab::Map(tab) => {
                        tab.ui(ui, self.renderer);
                    }
                    Tab::ActivityList(tab) => {
                        let res = tab.ui(ui);
                        self.process_result(res);
                    }
                    Tab::Activity(tab) => {
                        tab.ui(ui, self.renderer);
                    } // Tab::TestScene(tab) => {
                      //     tab.ui(ui, self.renderer);
                      // }
                      // Tab::TagLookup(data) => {
                      //     self.process_result(data.ui(ui));
                      // }
                });
            });
    }

    fn allowed_in_windows(&self, tab: &mut Self::Tab) -> bool {
        !tab.is_fixed()
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        !tab.is_fixed()
    }
}

impl<'a> TabViewer<'a> {
    fn process_result(&mut self, result: TabResult) {
        match result {
            TabResult::Continue => {}
            TabResult::Open(tab) => self.added_nodes.push(tab),
        }
    }
}

pub trait DockStateExt<Tab> {
    fn find_tab(&self, predicate: impl Fn(&Tab) -> bool) -> Option<TabPath>;
}

impl<Tab> DockStateExt<Tab> for DockState<Tab> {
    fn find_tab(&self, predicate: impl Fn(&Tab) -> bool) -> Option<TabPath> {
        for (si, surface) in self.iter_surfaces().enumerate() {
            for (ni, node) in surface.iter_nodes().enumerate() {
                for (ti, tab) in node.iter_tabs().enumerate() {
                    if predicate(tab) {
                        return Some(TabPath {
                            surface: si.into(),
                            node: ni.into(),
                            tab: ti.into(),
                        });
                    }
                }
            }
        }

        None
    }
}

pub enum TabResult {
    Continue,
    Open(Tab),
}
