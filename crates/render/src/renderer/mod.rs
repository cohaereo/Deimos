pub mod cascades;
mod fullscreen_pass;
pub mod globals;
pub mod hdri;
// pub mod immediate;
pub mod internal;
pub mod object;
pub mod packet;
pub mod scene;
pub mod shadow_mask;
pub mod ssr;

use std::sync::Arc;

use deimos_data::tfx::PrimitiveType;
use parking_lot::RwLock;
use slotmap::SlotMap;

use crate::{
    asset::AssetManager,
    gpu::{Gpu, command_list::CommandList},
    renderer::{
        globals::RenderGlobals,
        internal::InternalResources,
        object::{RenderObject, RenderObjectHandle},
    },
    tfx::{
        externs::{BaseExternSource, ExternContainer},
        technique::Technique,
    },
    util::thread_cell::ThreadMutCell,
};

pub struct Renderer {
    pub gpu: Arc<Gpu>,
    pub objects: RwLock<SlotMap<RenderObjectHandle, RenderObject>>,
    pub asset_manager: Arc<AssetManager>,
    pub globals: RenderGlobals,
    pub externs: ThreadMutCell<ExternContainer>,
    pub internal: InternalResources,
    // pub immediate: ImmediateRenderer,
}

impl Renderer {
    pub fn new(gpu: Arc<Gpu>) -> Arc<Self> {
        let asset_manager = Arc::new(AssetManager::new(&gpu));
        let renderer = Arc::new(Self {
            internal: InternalResources::new(&gpu).expect("Failed to create internal resources"),
            globals: RenderGlobals::load(&asset_manager, &gpu)
                .expect("Failed to load render globals"),
            asset_manager,
            // immediate: ImmediateRenderer::new(&gpu).expect("Failed to create immediate renderer"),
            gpu: gpu.clone(),
            objects: RwLock::new(SlotMap::with_key()),
            externs: ThreadMutCell::new(ExternContainer::default()),
        });
        *gpu.extern_source.write() = BaseExternSource::Renderer(renderer.clone());
        renderer
    }

    pub fn shutdown(&self) {
        self.objects.write().clear();
        self.asset_manager.shutdown();
    }

    pub fn execute_global_pipeline(
        &self,
        cmd: &mut CommandList,
        pipeline: &Technique,
        _name: &str,
    ) {
        // cmd_event_span!(cmd, &format!("[{name}]"));
        cmd.set_input_topology(PrimitiveType::TriangleStrip);
        cmd.set_input_layout(0);
        cmd.flush_states();

        pipeline.bind(cmd);

        cmd.draw(0..4, 0..1);
    }
}

impl Renderer {
    pub fn add_object(&self, object: RenderObject) -> RenderObjectHandle {
        self.objects.write().insert(object)
    }

    pub fn remove_object(&self, handle: RenderObjectHandle) {
        self.objects.write().remove(handle);
    }

    pub fn is_object_loaded(&self, handle: RenderObjectHandle) -> bool {
        self.objects
            .read()
            .get(handle)
            .is_some_and(|o| o.renderer.is_loaded())
    }
}
