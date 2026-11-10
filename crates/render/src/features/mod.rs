pub mod cubemap;
pub mod decals;
pub mod decorators;
pub mod dynamic_model;
pub mod light;
mod shared;
mod skinning;
pub mod static_instances;
pub mod terrain_patches;

use std::any::Any;
use std::sync::atomic::AtomicBool;

use deimos_data::tfx::{RenderStage, features::dynamic::RenderStageSubscription};
use glam::Vec3;

use crate::{
    gpu::command_list::CommandList,
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode, SubmitNodeContainer},
    },
    visibility::ViewVisibility,
};

pub trait FeatureRenderer: Send + Any {
    fn visibility_test(&mut self, visibility: &ViewVisibility) {
        _ = visibility;
    }

    fn populate_submit_node_blocks(
        &self,
        renderer: &Renderer,
        view_node: (usize, &RenderPerViewNode),
        frame_node: &RenderPerFrameNode,
        visibility: &ViewVisibility,
        submit_node_blocks: &mut SubmitNodeContainer,
    );

    // fn extract_per_frame(&mut self, renderer: &Renderer, frame_node: &RenderPerFrameNode);

    // fn extract_per_view(
    //     &mut self,
    //     renderer: &Renderer,
    //     frame_node: &RenderPerFrameNode,
    //     view_node: &RenderPerViewNode,
    // );

    fn prepare_per_frame(&self, cmd: &mut CommandList, frame_node: &RenderPerFrameNode) {
        _ = (cmd, frame_node);
    }

    fn prepare_per_view(
        &self,
        cmd: &mut CommandList,
        frame_node: &RenderPerFrameNode,
        view_node: &RenderPerViewNode,
    ) {
        _ = (cmd, frame_node, view_node);
    }

    fn submit(
        &self,
        cmd: &mut CommandList,
        stage: RenderStage,
        frame_node: &RenderPerFrameNode,
        view_node: &RenderPerViewNode,
        submit_key: u64,
    );

    // fn submit_parallel(
    //     &self,
    //     renderer: &Arc<Renderer>,
    //     view_index: usize,
    //     set: CommandListSetId,
    //     stage: RenderStage,
    //     jobs: &mut Vec<JobHandle>,
    // ) {
    //     _ = (renderer, view_index, set, stage, jobs);
    // }

    fn subscribed_stages(&self) -> RenderStageSubscription;

    /// Returns true if the feature renderer has finished loading any dependencies (techniques, buffers, etc)
    fn is_loaded(&self) -> bool {
        true
    }

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub const LOD_COUNT: u8 = u8::BITS as u8;
pub static LOD_ENABLED: AtomicBool = AtomicBool::new(true);

/// Distance-to-bounding-radius ratios at which each LOD level starts
const LOD_DISTANCE_RATIOS: [f32; LOD_COUNT as usize - 1] =
    [16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0];

pub(crate) const DETAIL_LOD_DISTANCE_RATIOS: [f32; 3] = [4.0, 8.0, 16.0];

pub(crate) fn lod_for_sphere_with_ratios(
    center: Vec3,
    radius: f32,
    viewer: Vec3,
    ratios: &[f32],
    max_lod: u8,
) -> u8 {
    let ratio = center.distance(viewer) / radius.max(1.0);
    let level = ratios
        .iter()
        .position(|&threshold| ratio < threshold)
        .unwrap_or(ratios.len()) as u8;
    level.min(max_lod)
}

/// Selects a LOD level from a bounding sphere and the viewer position.
pub fn lod_for_sphere(center: Vec3, radius: f32, viewer: Vec3, max_lod: u8) -> u8 {
    lod_for_sphere_with_ratios(center, radius, viewer, &LOD_DISTANCE_RATIOS, max_lod)
}
