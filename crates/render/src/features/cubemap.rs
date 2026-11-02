use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::{
    PrimitiveType, RenderStage,
    features::{
        cubemap::{CubemapShape, SCubemapComponent},
        dynamic::RenderStageSubscription,
    },
};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4, Vec4Swizzles};

use crate::{
    asset::{Handle, index_buffer::IndexBuffer, texture::Texture, vertex_buffer::VertexBuffer},
    features::rigid_model::DynamicObjectData,
    gpu::command_list::ConstantBufferView,
    renderer::{
        Renderer,
        packet::{RenderPerFrameNode, RenderPerViewNode},
    },
    tfx::externs::BaseExternSource,
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

        let (_, rotation, translation) = data.local_to_world.to_scale_rotation_translation();
        let local_to_world = Mat4::from_scale_rotation_translation(
            self.cubemap.volume_extents.xyz(),
            rotation,
            translation,
        );

        // TODO(cohae): Fade parameters need to be calculated from the cubemap data
        // const EPSILON: Vec4 = Vec4::splat(0.0001);
        // let fade_neg_scale = Vec4::select(
        //     self.cubemap.unk40_fade1.abs().cmpgt(EPSILON),
        //     Vec4::ONE / self.cubemap.unk40_fade1,
        //     Vec4::splat(1000.0),
        // );
        // let fade_pos_scale = Vec4::select(
        //     self.cubemap.unk50_fade2.abs().cmpgt(EPSILON),
        //     Vec4::ONE / self.cubemap.unk50_fade2,
        //     Vec4::splat(1000.0),
        // );
        let scale = self.cubemap.volume_extents.xyz() / 0.5;
        let bias = scale - Vec3::ONE;

        let [probe_res_x, probe_res_y, probe_res_z] = self.cubemap.probes_resolution;
        let grid = Vec3::new(probe_res_x as f32, probe_res_y as f32, probe_res_z as f32);
        let constants = CubemapPixelConstants {
            fade_neg: scale.extend(0.0),
            fade_pos: scale.extend(0.075),
            fade_neg_bias: bias.extend(20.0),
            fade_pos_bias: bias.extend(8.5),

            ..CubemapPixelConstants::new(local_to_world, rotation, grid)
        };

        let cb_ps = cmd
            .gpu()
            .frame()
            .upload
            .upload_bytes(bytemuck::bytes_of(&constants))
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

        let mut alpha = false;
        if let Some(t) = self.texture_cubemap_specular.get() {
            t.bind(cmd, 0, deimos_data::tfx::ShaderStage::Pixel);
        }
        if let Some(t) = self.texture_cubemap_alpha.get() {
            t.bind(cmd, 1, deimos_data::tfx::ShaderStage::Pixel);
            alpha = true;
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
            .get_specialized_cubemap_pipeline(CubemapShape::Cube, alpha, true, true, false)
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
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CubemapPixelConstants {
    pub fade_neg: Vec4, // cb11[0]
    pub fade_pos: Vec4, // cb11[1]

    pub fade_neg_bias: Vec4, // cb11[2], .w = intensity
    pub fade_pos_bias: Vec4, // cb11[3], .w = cube LOD

    pub unk4: Vec4,            // cb11[4]
    pub local_to_world: Mat4,  // cb11[5..=8], row-major 3x4
    pub world_to_local: Mat4,  // cb11[9..=11]
    pub unk13: Vec4,           // cb11[13]
    pub volume: Vec4,          // cb11[14], xyz = 1/grid, w = coverage
    pub volume_curve: Vec4,    // cb11[15], y/z/w
    pub sphere: Vec4,          // cb11[16]
    pub sphere_curve: Vec4,    // cb11[17]
    pub flags: Vec4,           // cb11[18], w = use raw radiance
    pub ambient_scale: Vec4,   // cb11[19]
    pub ambient_add: Vec4,     // cb11[20]
    pub parallax_center: Vec4, // cb11[21]
    pub parallax_min: Vec4,    // cb11[22]
    pub parallax_max: Vec4,    // cb11[23]
    pub unk24: Vec4,
    pub unk25: Vec4,
    pub unk26: Vec4,
    pub unk27: Vec4,
}

impl CubemapPixelConstants {
    pub fn new(local_to_world: Mat4, rotation: Quat, grid_resolution: Vec3) -> Self {
        let grid = grid_resolution.max(Vec3::ONE);

        Self {
            // No spatial fade.
            fade_neg: Vec4::ZERO,
            fade_pos: Vec4::ZERO,
            fade_neg_bias: Vec4::new(0.0, 0.0, 0.0, 1.0),
            fade_pos_bias: Vec4::new(0.0, 0.0, 0.0, 0.0),
            unk4: Vec4::ZERO,

            local_to_world: Mat4::from_cols(
                local_to_world.row(0).truncate().extend(0.0),
                local_to_world.row(1).truncate().extend(0.0),
                local_to_world.row(2).truncate().extend(0.0),
                local_to_world.w_axis,
            ),
            world_to_local: {
                let r = Mat3::from_quat(rotation.conjugate());
                Mat4::from_cols(
                    r.row(0).extend(0.0),
                    r.row(1).extend(0.0),
                    r.row(2).extend(0.0),
                    Vec4::ZERO,
                )
            },
            unk13: Vec4::ZERO,

            volume: Vec4::new(1.0 / grid.x, 1.0 / grid.y, 1.0 / grid.z, 1.0),
            volume_curve: Vec4::new(0.0, 0.0, 1.0, 1.0),

            sphere: Vec4::ZERO, // .w = 0 -> sphere path disabled
            sphere_curve: Vec4::ZERO,

            flags: Vec4::new(0.0, 0.0, 0.0, 1.0),
            ambient_scale: Vec4::new(1.0, 1.0, 1.0, 0.0),
            ambient_add: Vec4::ZERO,

            parallax_center: Vec4::ZERO,
            parallax_min: Vec4::new(-1.0, -1.0, -1.0, 0.0),
            parallax_max: Vec4::new(1.0, 1.0, 1.0, 0.0),
            unk24: Vec4::ZERO,
            unk25: Vec4::ZERO,
            unk26: Vec4::ZERO,
            unk27: Vec4::ZERO,
        }
    }
}
