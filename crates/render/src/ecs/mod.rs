use ahash::HashSet;
use deimos_data::tfx::{FeatureRendererSubscription, geometry::AxisAlignedBBox};
use deimos_ecs::{
    object::{ObjectChannels, PermutationConfig},
    transform::Transform,
    visibility::Visible,
};
use glam::Vec3;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    ecs::render_objects::{DynamicRenderObject, StaticRenderObject},
    features::{
        dynamic_model::{DynamicModel, DynamicObjectData},
        light::{LightRenderer, SHADOWMAP_RESOLUTION},
    },
    gpu::{command_list::ConstantBufferView, render_target::DepthBuffer},
    renderer::{
        Renderer,
        packet::ViewPacket,
        scene::{SceneRenderer, ShadowView},
    },
    visibility::{ViewVisibility, frustum::Frustum},
};

pub mod render_objects;

#[profiling::function]
pub fn s_update_object_channels(world: &hecs::World) {
    for (_entity, (transform, object_channels)) in world
        .query::<(Option<&Transform>, &mut ObjectChannels)>()
        .iter()
    {
        object_channels.reset_usage_counters();
        object_channels.set_by_name(
            "interpolated_world_position",
            transform.map_or(Vec3::ZERO, |t| t.translation).extend(1.0),
        );
    }
}

#[profiling::function]
pub fn s_visibility_test(world: &mut hecs::World, visibility: &ViewVisibility) {
    let dynamic_entities: Vec<hecs::Entity> = world
        .query::<&DynamicRenderObject>()
        .iter()
        .map(|(entity, _)| entity)
        .collect();

    for e in dynamic_entities {
        _ = world.insert_one(e, Visible(false));
    }

    let mut query = world.query::<(
        Option<&Transform>,
        &DynamicRenderObject,
        Option<&AxisAlignedBBox>,
        &mut Visible,
    )>();
    let nodes = query
        .iter()
        .map(|(_e, (transform, _render_object, bounds, visible))| (transform, bounds, visible))
        .collect::<Vec<_>>();

    nodes
        .into_par_iter()
        .for_each(|(transform, bounds, visible)| {
            let transform = transform.copied().unwrap_or_default();
            let bounds = bounds.map_or(AxisAlignedBBox::EVERYTHING, |b| {
                b.transformed(transform.local_to_world())
            });

            visible.0 = visibility.is_visible(&bounds);
        });
}

#[profiling::function]
pub fn s_extract_frame_packet(
    world: &hecs::World,
    scene: &mut SceneRenderer,
    visibility: &ViewVisibility,
    features: FeatureRendererSubscription,
    sun_shadows: bool,
) {
    let SceneRenderer {
        frame_packet,
        main_view,
        shadow_views,
        parent: renderer,
        ..
    } = scene;

    let mut render_objects = renderer.objects.write();

    frame_packet.insert_view(0);
    if sun_shadows {
        for (i, shadow_view) in main_view.shadow_views.iter_mut().enumerate() {
            shadow_view.id = 1 + i;
            frame_packet.insert_view(shadow_view.id);
        }
    }

    // Register shadow maps for shadowing lights that have not been rendered yet
    let mut next_light_shadow_view_id = 1 + main_view.shadow_views.len();
    if renderer.asset_manager.is_idle() {
        for (_entity, render_object) in world.query::<&DynamicRenderObject>().iter() {
            let object = render_object.handle();

            if shadow_views
                .iter()
                .any(|view| !view.rendered && view.object == object)
            {
                continue;
            }

            let Some(light_renderer) = render_objects
                .get_mut(object)
                .and_then(|render_object| render_object.get_mut::<LightRenderer>())
            else {
                continue;
            };
            let Some(shadow_camera) = light_renderer.pending_shadow_camera() else {
                continue;
            };

            let depth =
                match DepthBuffer::new(&renderer.gpu, (SHADOWMAP_RESOLUTION, SHADOWMAP_RESOLUTION))
                {
                    Ok(depth) => depth,
                    Err(e) => {
                        error!("Failed to create light shadow map: {e:?}");
                        continue;
                    }
                };

            let view_id = next_light_shadow_view_id;
            next_light_shadow_view_id += 1;

            frame_packet.insert_view(view_id);
            shadow_views.push(ShadowView {
                id: view_id,
                object,
                depth: Some(depth),
                world_to_camera: shadow_camera.world_to_camera,
                camera_to_projective: shadow_camera.camera_to_projective,
                far_plane: shadow_camera.far_plane,
                frustum: Frustum::from_view_and_projection(
                    shadow_camera.world_to_camera,
                    shadow_camera.camera_to_projective,
                ),
                rendered: false,
            });
        }
    }

    let views = [&main_view];
    let pending_shadow_views = shadow_views
        .iter()
        .filter(|view| !view.rendered)
        .collect_vec();

    for (_entity, (static_render_object, bounds)) in world
        .query::<(&StaticRenderObject, Option<&AxisAlignedBBox>)>()
        .iter()
    {
        if !features.is_subscribed(render_objects[static_render_object.handle()].feature_type) {
            continue;
        }

        let bounds = bounds.map_or(AxisAlignedBBox::EVERYTHING, |b| *b);
        let frame_node = frame_packet.push_frame_node::<()>(static_render_object.handle(), None);

        for (view_id, _v) in views.iter().enumerate() {
            if visibility.is_visible(&bounds) {
                frame_packet.push_view_node::<()>(view_id, frame_node, None);
            }
        }

        if sun_shadows {
            for v in main_view.shadow_views.iter() {
                if v.frustum.aabb_intersecting(&bounds) {
                    frame_packet.push_view_node::<()>(v.id, frame_node, None);
                }
            }
        }

        for v in &pending_shadow_views {
            if v.frustum.aabb_intersecting(&bounds) {
                frame_packet.push_view_node::<()>(v.id, frame_node, None);
            }
        }
    }

    for (_entity, (transform, render_object, permutations, object_channels, visible)) in world
        .query::<(
            Option<&Transform>,
            &DynamicRenderObject,
            Option<&PermutationConfig>,
            Option<&ObjectChannels>,
            Option<&Visible>,
        )>()
        .iter()
    {
        if !features.is_subscribed(render_objects[render_object.handle()].feature_type) {
            continue;
        }

        let transform = transform.copied().unwrap_or_default();
        let permutation = if let Some(permutation) = permutations {
            permutation
                .calculate_permutation_index()
                .unwrap_or(render_object.permutation)
        } else {
            render_object.permutation
        };

        if let Some(object_channels) = object_channels
            && let Some(dynamic_model) = render_objects
                .get_mut(render_object.handle())
                .and_then(|r| r.get_mut::<DynamicModel>())
        {
            for channel in &object_channels.0 {
                if let Some(target) = dynamic_model.channels.get_mut(&channel.name) {
                    target.value = channel.value;
                    target.expression.clone_from(&channel.expression);
                    target.usage.clone_from(&channel.usage);
                } else {
                    dynamic_model.channels.insert(channel.name, channel.clone());
                }
            }
        }

        let frame_node = frame_packet.push_frame_node(
            render_object.handle(),
            Some(DynamicObjectData {
                local_to_world: transform.local_to_world(),
                permutation,
                cbuffer: ConstantBufferView { addr: 0, size: 0 },
                _pad: 0,
            }),
        );

        for (view_id, _v) in views.iter().enumerate() {
            if !visible.is_none_or(|v| v.0) {
                continue;
            }

            frame_packet.push_view_node::<()>(view_id, frame_node, None);
        }
    }
}

#[profiling::function]
pub fn node_visibility_test(
    scene: &mut SceneRenderer,
    visibility: &ViewVisibility,
    view_id: usize,
) {
    let SceneRenderer {
        parent: renderer,
        frame_packet,
        ..
    } = scene;

    let mut render_objects = renderer.objects.write();
    let Some(ViewPacket { view_nodes, .. }) = frame_packet.views.get(view_id) else {
        error!("Invalid view id {view_id}");
        return;
    };

    let tested: HashSet<_> = view_nodes
        .iter()
        .map(|v| frame_packet.per_frame_nodes[v.frame_node].object)
        .collect();

    render_objects
        .iter_mut()
        .filter(|(key, _)| tested.contains(key))
        .map(|(_, render_object)| render_object)
        .collect::<Vec<_>>()
        .into_par_iter()
        .for_each(|render_object| render_object.renderer.visibility_test(visibility));
}

pub fn populate_submit_nodes(
    scene: &mut SceneRenderer,
    visibility: &ViewVisibility,
    view_id: usize,
) {
    let SceneRenderer {
        parent: renderer,
        frame_packet,
        ..
    } = scene;

    let render_objects = renderer.objects.read();
    let Some(ViewPacket {
        view_nodes,
        submit_node_blocks,
        ..
    }) = frame_packet.views.get_mut(view_id)
    else {
        error!("invalid view id {view_id}");
        return;
    };

    for (view_node_index, view_node, frame_node, render_object) in view_nodes
        .iter()
        .enumerate()
        .map(|(view_node, v)| (view_node, v, &frame_packet.per_frame_nodes[v.frame_node]))
        .map(|(view_node, v, o)| (view_node, v, o, &render_objects[o.object]))
    {
        render_object.renderer.populate_submit_node_blocks(
            renderer,
            (view_node_index, view_node),
            frame_node,
            visibility,
            submit_node_blocks,
        );
    }

    {
        profiling::scope!("sort stages");
        for stage in submit_node_blocks.blocks_mut() {
            stage.sort_by_key(|a| a.key);
        }
    }
}

pub fn s_are_all_objects_loaded(world: &hecs::World, renderer: &Renderer) -> bool {
    for (_entity, static_render_object) in world.query::<&StaticRenderObject>().iter() {
        if !renderer.is_object_loaded(static_render_object.handle()) {
            return false;
        }
    }

    for (_entity, render_object) in world.query::<&DynamicRenderObject>().iter() {
        if !renderer.is_object_loaded(render_object.handle()) {
            return false;
        }
    }

    true
}
