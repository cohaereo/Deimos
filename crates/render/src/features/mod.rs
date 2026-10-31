pub mod decals;
pub mod decorators;
pub mod light;
pub mod rigid_model;
mod shared;
mod skinning;
pub mod static_instances;
pub mod terrain_patches;

use std::any::Any;

use deimos_data::tfx::{RenderStage, features::dynamic::RenderStageSubscription};

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
