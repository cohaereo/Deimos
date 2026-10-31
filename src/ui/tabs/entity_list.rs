use std::{collections::BTreeMap, sync::Arc};

use deimos_data::{
    map::ComponentData,
    pattern::{SComponent, SPattern},
    tag::TagRef,
};
use deimos_ecs::{
    transform::Transform,
    world::pattern::{spawn_pattern, spawn_pattern_from_header},
};
use deimos_render::renderer::Renderer;
use egui::Ui;
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::{TagHash, package_manager};

use super::TabResult;
use crate::{
    app::SharedState,
    ui::{
        GuiRenderer,
        tabs::model_list::{ModelEntry, ModelListBase, ModelProvider},
    },
    world::pattern::load_component,
};

pub struct EntityListTab {
    base: ModelListBase<EntityModelProvider>,
}

impl EntityListTab {
    pub fn new(renderer: &Arc<Renderer>, state: &Arc<SharedState>) -> Self {
        Self {
            base: ModelListBase::new(renderer, state, EntityModelProvider::new(renderer.clone())),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, gui_renderer: &mut GuiRenderer) -> TabResult {
        self.base.ui(ui, gui_renderer)
    }
}

struct EntityModelProvider {
    renderer: Arc<Renderer>,
    package_keys: Vec<u16>,
    packages: BTreeMap<u16, (Vec<ModelEntry>, usize)>,
}

impl EntityModelProvider {
    fn new(renderer: Arc<Renderer>) -> Self {
        let packages: BTreeMap<u16, _> = package_manager()
            .package_paths
            .keys()
            .filter_map(|id| {
                let num_entities = package_manager().lookup.tag32_entries_by_pkg[id]
                    .iter()
                    .filter(|e| e.reference == SPattern::ID.unwrap())
                    .count();

                if num_entities > 0 {
                    Some((*id, (vec![], num_entities)))
                } else {
                    None
                }
            })
            .collect();

        Self {
            renderer,
            package_keys: packages.keys().cloned().collect(),
            packages,
        }
    }
}

impl ModelProvider for EntityModelProvider {
    fn name(&self) -> &str {
        "entities"
    }

    fn package_keys(&self) -> &[u16] {
        &self.package_keys
    }

    fn package(&self, pkg_id: u16) -> Option<&[ModelEntry]> {
        self.packages
            .get(&pkg_id)
            .map(|(entries, _)| entries.as_slice())
    }

    fn package_mut(&mut self, pkg_id: u16) -> Option<&mut [ModelEntry]> {
        self.packages
            .get_mut(&pkg_id)
            .map(|(entries, _)| entries.as_mut_slice())
    }

    fn num_models(&self, pkg_id: u16) -> usize {
        self.packages
            .get(&pkg_id)
            .map_or(0, |&(_, num_models)| num_models)
    }

    fn load_model(
        &mut self,
        hash: TagHash,
        world: &mut hecs::World,
    ) -> anyhow::Result<hecs::Entity> {
        spawn_pattern(
            world,
            hash,
            None,
            None,
            |world, entity, pattern, data, component| {
                load_component(&self.renderer, world, entity, pattern, data, component)
            },
        )
    }

    fn load_package(&mut self, pkg_id: u16) {
        let Some((entries, _)) = self.packages.get_mut(&pkg_id) else {
            return;
        };
        if !entries.is_empty() {
            return;
        }

        *entries = package_manager().lookup.tag32_entries_by_pkg[&pkg_id]
            .iter()
            .enumerate()
            .filter(|(_, e)| e.reference == SPattern::ID.unwrap())
            .filter_map(|(i, _)| {
                let hash = TagHash::new(pkg_id, i as u16);

                match package_manager().read_tag_struct::<SPattern>(hash) {
                    Ok(pattern) => {
                        let cb = |world: &mut hecs::World,
                                  entity: hecs::Entity,
                                  pattern: &SPattern,
                                  data: &ComponentData,
                                  component: &TagRef<SComponent>| {
                            load_component(&self.renderer, world, entity, pattern, data, component)
                        };

                        let mut world = hecs::World::new();
                        if let Err(e) = spawn_pattern_from_header(
                            &mut world,
                            &pattern,
                            None,
                            Some(Transform::default()),
                            &cb,
                        ) {
                            error!("Failed to load pattern {hash}: {e:?}");
                        }

                        Some(ModelEntry {
                            hash,
                            thumbnail_world: Some(world),
                            thumbnail: None,
                            rerender_needed: false,
                        })
                    }
                    Err(err) => {
                        error!("Failed to read pattern tag {hash}: {err}",);
                        None
                    }
                }
            })
            .collect();
    }

    fn unload_package(&mut self, pkg_id: u16) {
        if let Some((entries, _)) = self.packages.get_mut(&pkg_id) {
            entries.clear();
        }
    }
}
