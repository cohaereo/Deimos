use std::{
    io::{Cursor, Seek, SeekFrom},
    sync::Arc,
};

use anyhow::Context;
use deimos_data::{
    hash::FNV1_BASE,
    map::{ComponentData, S808085E3},
    pattern::{SComponent, SPattern},
    tag::{OptionalTagRef, TagRef},
    tfx::{
        TfxFeatureRenderer,
        features::{
            dynamic::SDynamicModelComponent, light::SShadowingLight, statics::SUnk808082D5,
        },
        geometry::AxisAlignedBBox,
    },
};
use deimos_ecs::{
    object::PermutationConfig, transform::Transform, world::map::ComponentLoadResult,
};
use deimos_render::{
    ecs::render_objects::{DynamicRenderObject, StaticRenderObject},
    features::{
        decals::DecalCollectionRenderer, decorators::DecoratorRenderer, light::LightRenderer,
        rigid_model::DynamicModel, static_instances::StaticInstancesRenderer,
        terrain_patches::TerrainPatchesRenderer,
    },
    renderer::{Renderer, object::RenderObject},
};
use glam::{Vec3, Vec4Swizzles};
use itertools::multizip;
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::package_manager;

#[macro_export]
macro_rules! once {
    () => {{
        static RAN_ONCE: AtomicBool = AtomicBool::new(false);
        !RAN_ONCE.swap(true, Ordering::SeqCst)
    }};
}

pub fn load_component(
    renderer: &Arc<Renderer>,
    world: &mut hecs::World,
    entity: hecs::Entity,
    _pattern: &SPattern,
    data: &ComponentData,
    component: &TagRef<SComponent>,
) -> anyhow::Result<ComponentLoadResult> {
    macro_rules! get_component_data {
        ($type:ident) => {
            if let ComponentData::$type(c) = data {
                c
            } else {
                error!(
                    "Expected component data type {} for component type 0x{:08X}, found \
                         {}/0x{:08X}",
                    stringify!($type),
                    component.default_instance.resource_type,
                    data.class_name(),
                    data.class_id()
                );
                return Ok(ComponentLoadResult::Skipped);
            }
        };
    }

    match component.default_instance.resource_type {
        0x80808673 => {
            let mut cur = Cursor::new(package_manager().read_tag(component.taghash())?);
            cur.seek(SeekFrom::Start(component.definition.offset))?;
            let model: SDynamicModelComponent = TigerReadable::read_ds(&mut cur)
                .context("while reading dynamic model component")?;

            if let Some(permutations) = PermutationConfig::from_model(&model) {
                world.insert_one(entity, permutations)?;
            }

            let model = DynamicModel::load(
                renderer,
                model.model_hash,
                model.technique_map,
                model.techniques,
            )
            .context("while loading dynamic model")?;
            world.insert_one(
                entity,
                AxisAlignedBBox::from_center_extents(
                    model.model.model_offset.xyz(),
                    model.model.model_scale.xyz() * 2.0,
                ),
            )?;

            let default_permutation = model.default_permutation;
            let obj =
                renderer.add_object(RenderObject::new(TfxFeatureRenderer::RigidObject, model));
            let mut obj_component = DynamicRenderObject::new(renderer, obj);
            obj_component.permutation = default_permutation;
            world.insert_one(entity, obj_component)?;
        }
        0x808081A1 => {
            let data = get_component_data!(SWaterPlaneComponent);

            let model = DynamicModel::load(renderer, data.model, vec![], vec![])?;
            world.insert_one(entity, data.bounds)?;

            let obj = renderer.add_object(RenderObject::new(TfxFeatureRenderer::Water, model));
            let obj_component = DynamicRenderObject::new(renderer, obj);
            world.insert_one(entity, obj_component)?;
        }
        // 0x80808412 => {
        //     let mut cur = Cursor::new(package_manager().read_tag(component.taghash())?);
        //     cur.seek(SeekFrom::Start(component.unk18.offset + 0x88))?;
        //     let array: Vec<S8080841B> = TigerReadable::read_ds(&mut cur)?;

        //     for v1 in array {
        //         for v2 in v1.unk30 {
        //             if let Err(e) =
        //                 spawn_pattern(renderer, world, v2.entity.hash32(), None, None)
        //             {
        //                 error!(
        //                     "Failed to spawn nested pattern {:?}/{} in pattern component {}: \
        //                      {:?}",
        //                     v2.entity,
        //                     v2.entity.hash32(),
        //                     component.taghash(),
        //                     e
        //                 );
        //             }
        //         }
        //     }
        // }
        0x80804030 => {
            let data = get_component_data!(SMaterialPermutationsComponent);

            if let Ok(mut config) = world.get::<&mut PermutationConfig>(entity) {
                let mut cur = Cursor::new(package_manager().read_tag(component.taghash())?);
                cur.seek(SeekFrom::Start(component.definition.offset + 0xD8))?;
                let default_keys_order: Vec<S808085E3> = TigerReadable::read_ds(&mut cur)?;

                for (key, value) in &data.config {
                    let value = if *value == FNV1_BASE
                        && let Some(defaults) =
                            default_keys_order.iter().find(|item| item.key == *key)
                    {
                        *defaults
                            .values
                            .iter()
                            .find(|v| config.is_valid_value(*key, **v))
                            .unwrap_or(value)
                    } else {
                        *value
                    };

                    // if let Some(values) = config.get_available_values(*key)
                    //     && !values.contains(&value)
                    // {
                    //     value = *values.iter().next().unwrap_or(&value);
                    // }

                    config.configuration.insert(*key, value);
                }
            } else {
                debug!(
                    "Material permutations component found in component {}, but entity does not \
                             have a permutation config set?",
                    component.taghash()
                );
            }
        }
        0x80808562 => {
            let data = get_component_data!(SStaticTerrainPatchesComponent);

            let feature_renderer =
                TerrainPatchesRenderer::load(renderer, data.terrain, data.identifier)?;
            let bounds = feature_renderer.bounds();

            world.insert(
                entity,
                (
                    StaticRenderObject::new(
                        renderer,
                        renderer.add_object(RenderObject::new(
                            TfxFeatureRenderer::TerrainPatch,
                            feature_renderer,
                        )),
                    ),
                    bounds,
                ),
            )?;
        }
        0x808085AF => {
            let data = get_component_data!(SStaticInstancesCollectionComponent);
            let instances: SUnk808082D5 = package_manager().read_tag_struct(data.instances)?;
            world.insert_one(
                entity,
                StaticRenderObject::new(
                    renderer,
                    renderer.add_object(RenderObject::new(
                        TfxFeatureRenderer::ChunkedInstanceObjects,
                        Box::new(StaticInstancesRenderer::load_from_tag(
                            renderer,
                            instances.instances,
                        )?),
                    )),
                ),
            )?;
        }
        0x80808220 => {
            let data = get_component_data!(SDecalCollectionComponent);
            if let Some(collection) = &*data.decals {
                let decal_renderer = DecalCollectionRenderer::load(renderer, collection.clone())
                    .with_context(|| {
                        format!("loading decal collection {}", data.decals.taghash())
                    })?;
                world.insert_one(
                    entity,
                    StaticRenderObject::new(
                        renderer,
                        renderer.add_object(RenderObject::new(
                            TfxFeatureRenderer::DynamicDecals,
                            decal_renderer,
                        )),
                    ),
                )?;
            }
        }
        0x808085A9 => {
            let data = get_component_data!(SDecoratorsComponent);
            if let Some(decorators) = data.decorators.0.as_ref() {
                let decorator_renderer = DecoratorRenderer::load(
                    renderer,
                    data.decorators.taghash(),
                    decorators.clone(),
                )?;
                world.insert_one(
                    entity,
                    StaticRenderObject::new(
                        renderer,
                        renderer.add_object(RenderObject::new(
                            TfxFeatureRenderer::SpeedtreeTrees,
                            Box::new(decorator_renderer),
                        )),
                    ),
                )?;
            }
        }
        0x80808377 => {
            let data = get_component_data!(SSkyObjectCollectionComponent);
            let Some(objects) = &*data.objects else {
                return Ok(ComponentLoadResult::Skipped);
            };
            for obj in &objects.unk8 {
                // cohae: Objects with unk70 set to 5 are a solid red? These don't show up in-game
                if obj.unk70 == 5 {
                    continue;
                }

                let (scale, rotation, translation) = obj.transform.to_scale_rotation_translation();

                let render_obj = RenderObject::new(
                    TfxFeatureRenderer::SkyTransparent,
                    DynamicModel::load(renderer, obj.model_ref.entity_model, vec![], vec![])?,
                );

                // TODO(cohae): Again, spawning new entities for each object is kinda dumb
                world.spawn((
                    Transform::new(translation, rotation, scale),
                    DynamicRenderObject::new(renderer, renderer.add_object(render_obj)),
                ));
            }
        }
        0x80808543 => {
            let data = get_component_data!(SShadowingLightComponent);
            let light = if let Some(light) = data.light.0.as_ref() {
                light.clone()
            } else {
                let mut cur = Cursor::new(package_manager().read_tag(component.taghash())?);
                cur.seek(SeekFrom::Start(component.definition.offset + 0x130))?;
                let light = OptionalTagRef::<SShadowingLight>::read_ds(&mut cur)?;

                let Some(light) = light.0 else {
                    return Ok(ComponentLoadResult::Loaded);
                };
                light
            };

            let _transform = world
                .get::<&Transform>(entity)
                .clone()
                .map(|c| *c)
                .unwrap_or_default();
            // let shadowmap = ShadowMap::create(
            //     transform,
            //     (light.half_fov * 2.0).to_degrees(),
            //     0.5,
            //     light.far_plane,
            // );

            let mut light_renderer = LightRenderer::new_shadowing(renderer, &light)
                .context("reading shadowing light")?;
            let bb = light_renderer.calculate_bounds();

            // let mut view = View::new_shadow(
            //     format!("shadow_{}", data.light.taghash()),
            //     &renderer.gpu,
            //     (
            //         ShadowView::SHADOWMAP_RESOLUTION,
            //         ShadowView::SHADOWMAP_RESOLUTION,
            //     ),
            // )
            // .expect("Failed to create shadowmap view");

            // if once!() {
            //     warn!("Culling is disabled for shadow views");
            // }
            // view.disable_culling = true;

            // let ViewKind::Shadow(v) = &view.kind else {
            //     unreachable!("view is not a shadow view even though we just created it");
            // };

            // let surf = &v.shadow_map;
            // light_renderer.shadow_view =
            //     Some((surf.texture.clone(), surf.srv(0).unwrap().clone()));

            let render_obj = RenderObject::new(TfxFeatureRenderer::DeferredLights, light_renderer);

            world.insert(
                entity,
                (
                    DynamicRenderObject::new(renderer, renderer.add_object(render_obj)),
                    bb,
                    // shadowmap,
                ),
            )?;
        }
        0x80808334 => {
            let data = get_component_data!(SLightCollectionComponent);
            let Some(lights) = data.lights.0.as_ref() else {
                return Ok(ComponentLoadResult::Skipped);
            };

            for (light, transform, bounds) in multizip((
                &lights.lights,
                &lights.transforms,
                &lights.occlusion_bounds.bounds,
            )) {
                let mut light_renderer = LightRenderer::new(renderer, light, bounds.bb)
                    .context("Failed to load light")?;
                let bb = light_renderer.calculate_bounds();
                let render_obj = renderer.add_object(RenderObject::new(
                    TfxFeatureRenderer::ChunkedLights,
                    light_renderer,
                ));

                // TODO(cohae): ChunkedLights need to be chunked like static geometry
                world.spawn((
                    Transform::new(transform.translation.xyz(), transform.rotation, Vec3::ONE),
                    DynamicRenderObject::new(renderer, render_obj),
                    bb,
                ));
            }
        }
        0x80807F3B => {
            let _data = get_component_data!(SCubemapComponent);

            //     let render_obj = RenderObject::new(
            //         TfxFeatureRenderer::Cubemaps,
            //         Box::new(CubemapRenderer::load(&renderer.gpu, data)?),
            //     );

            //     world.insert_one(
            //         entity,
            //         DynamicRenderObject::new(renderer.add_object(render_obj)),
            //     )?;
        }
        0x808085DD => {
            let data = get_component_data!(SUmbraTomeComponent);
            let Some(tomes) = &*data.tag else {
                tracing::error!("Missing tag for SUmbraTomeComponent");
                return Ok(ComponentLoadResult::Skipped);
            };

            if tomes.tome0.is_none() {
                // Orbit has a tome component with no tomes(?)
                return Ok(ComponentLoadResult::Loaded);
            }

            let tome0_data = package_manager()
                .read_tag(tomes.tome0)
                .context("failed to read tome0 data")?;
            let tome = umbra::Tome::load_from_buffer(&tome0_data);

            world.insert_one(entity, tome)?;
        }
        _u => return Ok(ComponentLoadResult::Skipped),
    }

    Ok(ComponentLoadResult::Loaded)
}
