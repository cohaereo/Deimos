pub mod cascades;
pub mod globals;
// pub mod immediate;
pub mod internal;
pub mod object;
pub mod packet;
pub mod scene;

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

        cmd.draw_instanced(0..4, 0..1);
    }

    // pub fn draw_hdri_background(&self, cmd: &mut CommandList) {
    //     cmd.set_input_topology(PrimitiveType::TriangleStrip);
    //     cmd.set_input_layout(0);
    //     cmd.flush_states();

    //     cmd.set_root_signature(&self.internal.rs_hdri_background);
    //     cmd.set_pipeline_state(&self.internal.pso_hdri_background);

    //     let descriptor_range = cmd.gpu().frame().descriptors.allocate(2);
    //     let Some(deferred_srv) = self.externs.deferred.deferred_depth.get_srv() else {
    //         return;
    //     };
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         deferred_srv.cpu_handle(),
    //         descriptor_range.cpu_handle(0),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         self.internal.hdri.srv.cpu_handle(),
    //         descriptor_range.cpu_handle(1),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );

    //     let Some(view_scope_cbv) = cmd.get_shader_constant_buffer_view(ShaderStage::Pixel, 12)
    //     else {
    //         return;
    //     };

    //     cmd.set_graphics_root_descriptor_table(0, descriptor_range.gpu_handle(0));
    //     cmd.set_graphics_root_constant_buffer_view(1, view_scope_cbv);

    //     cmd.draw_instanced(0..4, 0..1);
    // }

    // pub fn apply_hdri_light(&self, cmd: &mut CommandList, cascades: &[(Mat4, ResourceView)]) {
    //     assert_eq!(cascades.len(), 4);
    //     cmd.set_input_topology(PrimitiveType::TriangleStrip);
    //     cmd.set_input_layout(0);
    //     cmd.flush_states();

    //     cmd.set_root_signature(&self.internal.rs_hdri_lighting);
    //     cmd.set_pipeline_state(&self.internal.pso_hdri_lighting);

    //     let descriptor_range = cmd.gpu().frame().descriptors.allocate(8);
    //     let Some(gbuffer_normal) = self.externs.deferred.deferred_rt1.get_srv() else {
    //         return;
    //     };
    //     let Some(gbuffer_third) = self.externs.deferred.deferred_rt2.get_srv() else {
    //         return;
    //     };
    //     let Some(deferred_depth_srv) = self.externs.deferred.deferred_depth.get_srv() else {
    //         return;
    //     };
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         gbuffer_normal.cpu_handle(),
    //         descriptor_range.cpu_handle(0),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         gbuffer_third.cpu_handle(),
    //         descriptor_range.cpu_handle(1),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         deferred_depth_srv.cpu_handle(),
    //         descriptor_range.cpu_handle(2),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         self.internal.hdri.srv.cpu_handle(),
    //         descriptor_range.cpu_handle(3),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     for (i, (_, srv)) in cascades.iter().enumerate() {
    //         cmd.gpu().copy_descriptors_simple(
    //             1,
    //             srv.cpu_handle(),
    //             descriptor_range.cpu_handle(4 + i),
    //             d3d12::DescriptorHeapType::CbvSrvUav,
    //         );
    //     }

    //     let Some(view_scope_cbv) = cmd.get_shader_constant_buffer_view(ShaderStage::Pixel, 12)
    //     else {
    //         return;
    //     };

    //     let cascade_matrices: Vec<Mat4> = cascades.iter().map(|(m, _)| *m).collect();
    //     let Ok(cascade_scope_cbv) = cmd
    //         .upload_ring()
    //         .upload_bytes(bytemuck::cast_slice(&cascade_matrices))
    //     else {
    //         error!("Failed to upload shadow cascade matrices");
    //         return;
    //     };

    //     cmd.set_graphics_root_descriptor_table(0, descriptor_range.gpu_handle(0));
    //     cmd.set_graphics_root_constant_buffer_view(1, cascade_scope_cbv);
    //     cmd.set_graphics_root_constant_buffer_view(2, view_scope_cbv);

    //     cmd.draw_instanced(0..4, 0..1);
    // }

    // pub fn generate_shadow_mask(&self, cmd: &mut CommandList, cascades: &[(Mat4, ResourceView)]) {
    //     assert_eq!(cascades.len(), 4);
    //     cmd.set_input_topology(PrimitiveType::TriangleStrip);
    //     cmd.set_input_layout(0);
    //     cmd.flush_states();

    //     cmd.set_root_signature(&self.internal.rs_generate_shadow_mask);
    //     cmd.set_pipeline_state(&self.internal.pso_generate_shadow_mask);

    //     let descriptor_range = cmd.gpu().frame().descriptors.allocate(5);
    //     let Some(deferred_depth_srv) = self.externs.deferred.deferred_depth.get_srv() else {
    //         return;
    //     };
    //     cmd.gpu().copy_descriptors_simple(
    //         1,
    //         deferred_depth_srv.cpu_handle(),
    //         descriptor_range.cpu_handle(0),
    //         d3d12::DescriptorHeapType::CbvSrvUav,
    //     );
    //     for (i, (_, srv)) in cascades.iter().enumerate() {
    //         cmd.gpu().copy_descriptors_simple(
    //             1,
    //             srv.cpu_handle(),
    //             descriptor_range.cpu_handle(1 + i),
    //             d3d12::DescriptorHeapType::CbvSrvUav,
    //         );
    //     }

    //     let Some(view_scope_cbv) = cmd.get_shader_constant_buffer_view(ShaderStage::Pixel, 12)
    //     else {
    //         return;
    //     };

    //     let cascade_matrices: Vec<Mat4> = cascades.iter().map(|(m, _)| *m).collect();
    //     let Ok(cascade_scope_cbv) = cmd
    //         .upload_ring()
    //         .upload_bytes(bytemuck::cast_slice(&cascade_matrices))
    //     else {
    //         error!("Failed to upload shadow cascade matrices");
    //         return;
    //     };

    //     cmd.set_graphics_root_descriptor_table(0, descriptor_range.gpu_handle(0));
    //     cmd.set_graphics_root_constant_buffer_view(1, cascade_scope_cbv);
    //     cmd.set_graphics_root_constant_buffer_view(2, view_scope_cbv);

    //     cmd.draw_instanced(0..4, 0..1);
    // }
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
