use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{
    PrimitiveType, RenderStage,
    features::{cubemap::SCubemapComponent, dynamic::RenderStageSubscription},
};
use glam::{Mat4, Quat, Vec3, Vec4};

use crate::{
    asset::{Handle, index_buffer::IndexBuffer, texture::Texture, vertex_buffer::VertexBuffer},
    features::rigid_model::DynamicObjectData,
    gpu::command_list::ConstantBufferView,
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode},
    },
    tfx::externs::{self, BaseExternSource},
    util::geometry,
};

use super::FeatureRenderer;
pub struct CubemapRenderer {
    // TODO(cohae): This should be a shared resource (eg. a struct in the renderer that we can use instead of recreating it for every light/cubemap)
    vb: VertexBuffer,
    ib: IndexBuffer,

    texture_cubemap_specular: Handle<Texture>,
    texture_cubemap_alpha: Handle<Texture>,
    texture_voxel_diffuse: Handle<Texture>,

    cubemap: SCubemapComponent,
}

impl CubemapRenderer {
    pub fn load(renderer: &Renderer, cubemap: &SCubemapComponent) -> anyhow::Result<Box<Self>> {
        let vb = VertexBuffer::load_data(
            &renderer.gpu,
            bytemuck::cast_slice(geometry::CUBE_VERTICES),
            size_of::<Vec3>() as u32,
        )?;
        let ib = IndexBuffer::load_data(
            &renderer.gpu,
            bytemuck::cast_slice(geometry::CUBE_INDICES),
            false,
        )?;

        Ok(Box::new(Self {
            vb,
            ib,

            texture_cubemap_specular: renderer
                .asset_manager
                .load(cubemap.texture_cube_specular_ibl),
            texture_cubemap_alpha: renderer.asset_manager.load(cubemap.texture_cube_alpha),
            texture_voxel_diffuse: renderer.asset_manager.load(cubemap.texture_voxel_diffuse),

            cubemap: cubemap.clone(),
        }))
    }
}

impl FeatureRenderer for CubemapRenderer {
    fn populate_submit_node_blocks(
        &self,
        _renderer: &Renderer,
        (view_node, _): (usize, &RenderPerViewNode),
        _frame_node: &RenderPerFrameNode,
        visibility: &crate::visibility::ViewVisibility,
        submit_node_blocks: &mut crate::renderer::packet::SubmitNodeContainer,
    ) {
        submit_node_blocks.broadcast(
            self.subscribed_stages() & visibility.render_stages,
            crate::renderer::packet::SubmitNode { view_node, key: 0 },
        );
    }

    fn submit(
        &self,
        cmd: &mut crate::gpu::command_list::CommandList,
        stage: deimos_data::tfx::RenderStage,
        frame_node: &crate::renderer::packet::RenderPerFrameNode,
        _view_node: &crate::renderer::packet::RenderPerViewNode,
        _submit_key: u64,
    ) {
        if stage != RenderStage::Cubemaps {
            return;
        }

        let renderer = match cmd.externs.base() {
            BaseExternSource::Renderer(renderer) => renderer.clone(),
            BaseExternSource::None => {
                error!("CubemapRenderer: No renderer found in externs base");
                return;
            }
        };

        let data = unsafe { frame_node.data::<DynamicObjectData>() }
            .expect("DynamicModel was extracted with no data!");

        let rotation = data.local_to_world.to_scale_rotation_translation().1;
        let rotation_neg = -rotation;

        cmd.externs.cubemaps = Some(
            externs::Cubemaps {
                specular_ibl: self.texture_cubemap_specular.clone().into(),
            }
            .into(),
        );

        let cb_vs = cmd
            .gpu()
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&CubemapTransform {
                translation: data
                    .local_to_world
                    .to_scale_rotation_translation()
                    .2
                    .extend(1.0),
                rotation: rotation_neg,
                scale: self.cubemap.volume_extents,
            }))
            .unwrap();

        let cb_ps = cmd
            .gpu()
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&CubemapPixelConstants {
                fade0: Vec4::ONE,
                fade1: Vec4::ONE,
                fade2: Vec4::ONE,
                fade3: Vec4::ONE,
                unk4: Vec4::ONE,
                unk5: Mat4::IDENTITY,
                unk9: Mat4::IDENTITY,
                unk13: Vec4::ONE,
                unk14: Vec4::ONE,
                unk15: Vec4::ONE,
                unk16: Vec4::ONE,
                unk17: Vec4::ONE,
                unk18: Vec4::ONE,
                unk19: Vec4::ONE,
                unk20: Vec4::ONE,
            }))
            .unwrap();

        cmd.set_shader_constant_buffer_view(
            deimos_data::tfx::ShaderStage::Vertex,
            11,
            Some(ConstantBufferView {
                addr: cb_vs.virtual_address(),
                size: cb_vs.len() as u64,
            }),
        );
        cmd.set_shader_constant_buffer_view(
            deimos_data::tfx::ShaderStage::Pixel,
            11,
            Some(ConstantBufferView {
                addr: cb_ps.virtual_address(),
                size: cb_ps.len() as u64,
            }),
        );

        if let Some(t) = self.texture_cubemap_specular.get() {
            t.bind(cmd, 0, deimos_data::tfx::ShaderStage::Pixel);
        }
        if let Some(t) = self.texture_cubemap_alpha.get() {
            t.bind(cmd, 1, deimos_data::tfx::ShaderStage::Pixel);
        }
        if let Some(t) = self.texture_voxel_diffuse.get() {
            t.bind(cmd, 2, deimos_data::tfx::ShaderStage::Pixel);
        }

        cmd.set_input_topology(PrimitiveType::Triangles);
        cmd.set_input_layout(1); // float3 v0 : POSITION0, // Format DXGI_FORMAT_R32G32B32_FLOAT size 12

        self.vb.bind_single(cmd, 0);
        self.ib.bind(cmd);

        renderer
            .globals
            .pipelines
            .cubemap_apply_cube_alpha_on_probes_on_relighting_on
            .bind(cmd);

        cmd.draw_indexed_instanced(0..geometry::CUBE_INDICES.len() as u32, 0..1, 0);
        cmd.flush_states();
    }

    fn subscribed_stages(&self) -> RenderStageSubscription {
        RenderStageSubscription::CUBEMAPS
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CubemapTransform {
    pub translation: Vec4,
    pub rotation: Quat,
    pub scale: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CubemapPixelConstants {
    pub fade0: Vec4, // cb11[0]
    pub fade1: Vec4, // cb11[1]
    pub fade2: Vec4, // cb11[2]
    pub fade3: Vec4, // cb11[3]
    pub unk4: Vec4,  // cb11[4]
    pub unk5: Mat4,  // cb11[5..=8]
    pub unk9: Mat4,  // cb11[9..=12]
    pub unk13: Vec4,
    pub unk14: Vec4, // cb11[14]
    pub unk15: Vec4, // cb11[15]
    pub unk16: Vec4, // cb11[16]
    pub unk17: Vec4, // cb11[17]
    pub unk18: Vec4, // cb11[18]
    pub unk19: Vec4, // cb11[19]
    pub unk20: Vec4, // cb11[20]
}
