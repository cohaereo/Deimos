use std::{collections::BTreeMap, sync::Arc};

use anyhow::Context;
use deimos_data::tfx::{
    TfxFeatureRenderer,
    features::statics::{SStaticInstanceTransform, SStaticMesh},
};
use deimos_ecs::transform::Transform;
use deimos_render::{
    ecs::render_objects::StaticRenderObject,
    features::static_instances::{StaticInstancesRenderer, StaticModelRenderer},
    gpu::alloc::staging::ImmutableStaging,
    renderer::{Renderer, object::RenderObject},
};
use egui::Ui;
use glam::{Quat, Vec3};
use hecs::Entity;
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::{TagHash, package_manager};

use super::TabResult;
use crate::{
    app::SharedState,
    ui::{
        GuiRenderer,
        tabs::model_list::{ModelEntry, ModelListBase, ModelProvider},
    },
};

pub struct StaticListTab {
    base: ModelListBase<StaticModelProvider>,
}

impl StaticListTab {
    pub fn new(renderer: &Arc<Renderer>, state: &Arc<SharedState>) -> Self {
        Self {
            base: ModelListBase::new(renderer, state, StaticModelProvider::new(renderer)),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, gui_renderer: &mut GuiRenderer) -> TabResult {
        self.base.ui(ui, gui_renderer)
    }
}

struct StaticModelProvider {
    package_keys: Vec<u16>,
    packages: BTreeMap<u16, (Vec<ModelEntry>, usize)>,
    renderer: Arc<Renderer>,
}

impl StaticModelProvider {
    fn new(renderer: &Arc<Renderer>) -> Self {
        let packages: BTreeMap<u16, _> = package_manager()
            .package_paths
            .keys()
            .filter_map(|id| {
                let num_statics = package_manager().lookup.tag32_entries_by_pkg[id]
                    .iter()
                    .filter(|e| e.reference == SStaticMesh::ID.unwrap())
                    .count();

                if num_statics > 0 {
                    Some((*id, (vec![], num_statics)))
                } else {
                    None
                }
            })
            .collect();

        Self {
            package_keys: packages.keys().cloned().collect(),
            packages,
            renderer: renderer.clone(),
        }
    }
}

impl ModelProvider for StaticModelProvider {
    fn name(&self) -> &str {
        "static_models"
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

    fn load_model(&mut self, hash: TagHash, world: &mut hecs::World) -> anyhow::Result<Entity> {
        load_static_mesh(&self.renderer, hash, world).context("Failed to load static model")
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
            .filter(|(_, e)| e.reference == SStaticMesh::ID.unwrap())
            .filter_map(|(i, _)| {
                let hash = TagHash::new(pkg_id, i as u16);
                let mut world = hecs::World::new();
                match load_static_mesh(&self.renderer, hash, &mut world) {
                    Ok(_entity) => Some(ModelEntry {
                        hash,
                        thumbnail_world: Some(world),
                        thumbnail: None,
                        rerender_needed: false,
                    }),
                    Err(err) => {
                        error!("Failed to load static model {hash}: {err}");
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

fn load_static_mesh(
    renderer: &Arc<Renderer>,
    hash: TagHash,
    world: &mut hecs::World,
) -> anyhow::Result<Entity> {
    let transform = SStaticInstanceTransform {
        rotation: Quat::IDENTITY,
        translation: Vec3::ZERO,
        scale: 1.0,
        unk20: [0, 0, 0, 0x3f800000],
        unk30: [0, 0, 0, 0x3f800000],
        unk40: [0, 0, 0, 0x3f800000],
        unk50: [0, 0, 0, 0x3f800000],
    };

    let data = package_manager().read_tag_struct::<SStaticMesh>(hash)?;

    let upload0 = ImmutableStaging::new(0);
    let model =
        StaticModelRenderer::new(renderer, vec![(transform, data.bounds)], hash, 0, &upload0)
            .context("Failed to load static model tag")?;
    // TODO(cohae): data.bounds seems to be mostly right, but needs to be double checked
    let entity = world.spawn((Transform::default(), model.bounds));
    let model_renderer = StaticInstancesRenderer::new(&renderer.gpu, vec![model], upload0);

    let obj = renderer.add_object(RenderObject::new(
        TfxFeatureRenderer::StaticObjects,
        Box::new(model_renderer),
    ));
    _ = world.insert_one(entity, StaticRenderObject::new(renderer, obj));

    Ok(entity)
}
