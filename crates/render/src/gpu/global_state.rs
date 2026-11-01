use std::mem::size_of;

use anyhow::Context;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use deimos_data::tfx::render_globals::SRenderGlobals;
use tiger_parse::PackageManagerExt;
use tiger_pkg::package_manager;

const DEPTH_STENCIL_COMBO_COUNT: usize = DEPTH_STENCIL_COMBOS.len();
const RASTERIZER_STATE_COUNT: usize = 9;
const DEPTH_BIAS_COUNT: usize = 9;

#[derive(Clone, Copy, Debug)]
pub struct BlendState {
    pub alpha_to_coverage_enable: bool,
    pub logic_op_enable: bool,
    pub logic_op: vk::LogicOp,
    pub render_targets: [vk::PipelineColorBlendAttachmentState; 8],
}

#[derive(Clone, Copy, Debug)]
pub struct RasterizerState {
    pub polygon_mode: vk::PolygonMode,
    pub cull_mode: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub depth_bias_enable: bool,
    pub depth_bias_constant_factor: f32,
    pub depth_bias_clamp: f32,
    pub depth_bias_slope_factor: f32,
    pub depth_clamp_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
}

impl Default for RasterizerState {
    fn default() -> Self {
        Self {
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            depth_bias_enable: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_clamp: 0.0,
            depth_bias_slope_factor: 0.0,
            depth_clamp_enable: false,
            multisample_enable: false,
            antialiased_line_enable: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DepthStencilState {
    pub depth_test_enable: bool,
    pub depth_write_enable: bool,
    pub depth_compare_op: vk::CompareOp,
    pub stencil_test_enable: bool,
    pub front: vk::StencilOpState,
    pub back: vk::StencilOpState,
}

impl Default for DepthStencilState {
    fn default() -> Self {
        Self {
            depth_test_enable: false,
            depth_write_enable: false,
            depth_compare_op: vk::CompareOp::ALWAYS,
            stencil_test_enable: false,
            front: vk::StencilOpState::default(),
            back: vk::StencilOpState::default(),
        }
    }
}

pub struct RenderStates {
    pub input_layouts: Vec<(
        Vec<vk::VertexInputBindingDescription>,
        Vec<vk::VertexInputAttributeDescription>,
    )>,
    pub blend_states: Vec<BlendState>,
    pub rasterizer_states: [[RasterizerState; RASTERIZER_STATE_COUNT]; DEPTH_BIAS_COUNT],
    pub depth_stencil_states: [(DepthStencilState, DepthStencilState); DEPTH_STENCIL_COMBO_COUNT],
}

impl RenderStates {
    const BLEND_STATE_DESC_DATA: &[u8] = include_bytes!("../../builtin/gpu/blend_states.bin");
    const RASTERIZER_STATE_DATA: &[u8] = include_bytes!("../../builtin/gpu/rasterizer_states.bin");
    const DEPTH_BIAS_DATA: &[u8] = include_bytes!("../../builtin/gpu/depth_biases.bin");

    const DEPTH_STATE_DATA: &[u8] = include_bytes!("../../builtin/gpu/depth_states.bin");
    const STENCIL_STATE_DATA: &[u8] = include_bytes!("../../builtin/gpu/stencil_states.bin");

    const REVERSED_DEPTH_FUNCS: [vk::CompareOp; 13] = [
        vk::CompareOp::ALWAYS,
        vk::CompareOp::ALWAYS,
        vk::CompareOp::LESS_OR_EQUAL,
        vk::CompareOp::LESS_OR_EQUAL,
        vk::CompareOp::GREATER_OR_EQUAL,
        vk::CompareOp::GREATER_OR_EQUAL,
        vk::CompareOp::GREATER,
        vk::CompareOp::LESS_OR_EQUAL,
        vk::CompareOp::LESS_OR_EQUAL,
        vk::CompareOp::ALWAYS,
        vk::CompareOp::NEVER,
        vk::CompareOp::ALWAYS,
        vk::CompareOp::LESS_OR_EQUAL,
    ];

    pub fn new(_device: &ash::Device) -> anyhow::Result<Self> {
        let mut input_layouts = vec![(vec![], vec![]); 255];
        for (i, layout) in BASE_INPUT_LAYOUTS.iter().enumerate() {
            input_layouts[i] = tiger_input_layout_to_vk(layout.elements);
        }

        let data: SRenderGlobals = package_manager().read_named_tag_struct("render_globals")?;
        let globs = &data.unk8.first().context("No render globals found")?.unk8.0;

        let element_set = &globs.input_layouts.elements_c;
        for l in globs.input_layouts.mapping.layouts.iter() {
            let mut layout_elements = vec![];
            for (buffer_index, &(element_index, is_instance_data)) in [
                (l.buffer_0, l.buffer_0_instanced),
                (l.buffer_1, l.buffer_1_instanced),
                (l.buffer_2, l.buffer_2_instanced),
                (l.buffer_3, l.buffer_3_instanced),
            ]
            .iter()
            .enumerate()
            {
                if element_index == u32::MAX {
                    continue;
                }
                for e in &element_set.sets[element_index as usize].elements {
                    let semantic = INPUT_SEMANTICS[e.semantic as usize];
                    let format = &INPUT_FORMATS[e.format as usize];
                    layout_elements.push(TigerInputLayoutElement {
                        _hlsl_type: format.hlsl_type,
                        format: format.format,
                        stride: format.stride,
                        semantic_name: semantic,
                        semantic_index: e.semantic_index as _,
                        buffer_index: buffer_index as _,
                        is_instance_data,
                    });
                }
            }

            input_layouts[l.index as usize] = tiger_input_layout_to_vk(&layout_elements);

            // println!("Layout {}", l.index);
            // for (ei, element) in layout_elements.iter().enumerate() {
            //     println!(
            //         " - {} v{ei} : {}{}, // Format {:?} size {}{}",
            //         element.hlsl_type,
            //         element.semantic_name,
            //         element.semantic_index,
            //         element.format,
            //         element._stride,
            //         if element.is_instance_data {
            //             " (instanced)"
            //         } else {
            //             ""
            //         }
            //     );
            // }
        }

        let blend_states = Self::parse_blend_states(Self::BLEND_STATE_DESC_DATA)?;
        let rasterizer_states =
            Self::parse_rasterizer_states(Self::RASTERIZER_STATE_DATA, Self::DEPTH_BIAS_DATA)?;
        let depth_stencil_states =
            Self::parse_depth_stencil_states(Self::DEPTH_STATE_DATA, Self::STENCIL_STATE_DATA)?;
        Ok(Self {
            input_layouts,
            blend_states,
            rasterizer_states,
            depth_stencil_states,
        })
    }

    fn parse_blend_states(data: &[u8]) -> anyhow::Result<Vec<BlendState>> {
        let raw_states: Vec<QuadBlendState> = bytemuck::allocation::pod_collect_to_vec(data);

        let states = raw_states
            .iter()
            .map(|raw| {
                let source_targets = raw.render_targets.map(|target| {
                    vk::PipelineColorBlendAttachmentState::default()
                        .blend_enable(target.blend_enable != 0)
                        .src_color_blend_factor(map_blend_factor(target.src_blend))
                        .dst_color_blend_factor(map_blend_factor(target.dest_blend))
                        .color_blend_op(map_blend_op(target.blend_op))
                        .src_alpha_blend_factor(map_blend_factor(target.src_blend_alpha))
                        .dst_alpha_blend_factor(map_blend_factor(target.dest_blend_alpha))
                        .alpha_blend_op(map_blend_op(target.blend_op_alpha))
                        .color_write_mask(vk::ColorComponentFlags::from_raw(u32::from(
                            target.render_target_write_mask,
                        )))
                });

                let mut render_targets = [source_targets[4]; 8];
                render_targets[..4].copy_from_slice(&source_targets[..4]);

                BlendState {
                    alpha_to_coverage_enable: false,
                    logic_op_enable: raw.render_targets[0].logic_op_enable != 0,
                    logic_op: map_logic_op(raw.render_targets[0].logic_op),
                    render_targets,
                }
            })
            .collect::<Vec<_>>();

        anyhow::ensure!(
            states.len() == 90,
            "unexpected blend state count {}",
            states.len()
        );
        Ok(states)
    }

    fn parse_rasterizer_states(
        rasterizer_data: &[u8],
        depth_bias_data: &[u8],
    ) -> anyhow::Result<[[RasterizerState; RASTERIZER_STATE_COUNT]; DEPTH_BIAS_COUNT]> {
        let raw_rasterizer_states: Vec<PaddedRasterizerState> =
            bytemuck::allocation::pod_collect_to_vec(rasterizer_data);
        let raw_depth_biases: Vec<ShortDepthBias> =
            bytemuck::allocation::pod_collect_to_vec(depth_bias_data);

        anyhow::ensure!(
            raw_rasterizer_states.len() == RASTERIZER_STATE_COUNT,
            "unexpected rasterizer state count {}",
            raw_rasterizer_states.len()
        );
        anyhow::ensure!(
            raw_depth_biases.len() == DEPTH_BIAS_COUNT,
            "unexpected depth bias state count {}",
            raw_depth_biases.len()
        );

        let base_states = raw_rasterizer_states
            .iter()
            .map(|raw| {
                let desc = &raw.desc;
                RasterizerState {
                    polygon_mode: map_fill_mode(desc.fill_mode),
                    cull_mode: map_cull_mode(desc.cull_mode),
                    front_face: if desc.front_counter_clockwise != 0 {
                        vk::FrontFace::COUNTER_CLOCKWISE
                    } else {
                        vk::FrontFace::CLOCKWISE
                    },
                    depth_bias_enable: false,
                    depth_bias_constant_factor: 0.0,
                    depth_bias_clamp: 0.0,
                    depth_bias_slope_factor: 0.0,
                    depth_clamp_enable: desc.depth_clip_enable == 0,
                    multisample_enable: desc.multisample_enable != 0,
                    antialiased_line_enable: desc.antialiased_line_enable != 0,
                }
            })
            .collect::<Vec<_>>();

        let mut states = [[RasterizerState::default(); RASTERIZER_STATE_COUNT]; DEPTH_BIAS_COUNT];
        for (bias_index, bias) in raw_depth_biases.iter().enumerate() {
            for (rasterizer_index, base) in base_states.iter().enumerate() {
                states[bias_index][rasterizer_index] = RasterizerState {
                    depth_bias_enable: bias.depth_bias != 0 || bias.slope_scaled_depth_bias != 0.0,
                    depth_bias_constant_factor: bias.depth_bias as f32,
                    depth_bias_clamp: bias.depth_bias_clamp,
                    depth_bias_slope_factor: bias.slope_scaled_depth_bias,
                    ..*base
                };
            }
        }

        Ok(states)
    }

    fn parse_depth_stencil_states(
        depth_data: &[u8],
        stencil_data: &[u8],
    ) -> anyhow::Result<[(DepthStencilState, DepthStencilState); DEPTH_STENCIL_COMBO_COUNT]> {
        let depth_states: Vec<ShortDepthState> =
            bytemuck::allocation::pod_collect_to_vec(depth_data);
        let stencil_states: Vec<ShortStencilDesc> =
            bytemuck::allocation::pod_collect_to_vec(stencil_data);

        // warn!("!!! STENCIL TESTING IS DISABLED");
        let states = std::array::from_fn(|index| {
            let (depth_index, stencil_index) = DEPTH_STENCIL_COMBOS[index];
            let depth = depth_states[depth_index];
            let stencil = stencil_states[stencil_index];
            let read_mask = stencil.read_mask;
            let write_mask = stencil.write_mask;

            let reverse = DepthStencilState {
                depth_test_enable: depth.depth_enable != 0,
                depth_write_enable: depth.depth_write_mask == 1,
                depth_compare_op: map_compare_op(depth.depth_func),
                stencil_test_enable: stencil.enabled != 0,
                front: map_stencil_op_state(stencil.front_face, read_mask, write_mask),
                back: map_stencil_op_state(stencil.back_face, read_mask, write_mask),
            };

            let forward = DepthStencilState {
                depth_compare_op: Self::REVERSED_DEPTH_FUNCS[depth_index],
                ..reverse
            };

            (reverse, forward)
        });

        Ok(states)
    }

    // fn create_input_layout(
    //     device: &d3d12::Device,
    //     elements: &[TigerInputLayoutElement],
    // ) -> anyhow::Result<d3d12::InputLayout> {
    //     let mut shader_input = "struct s_vs_in { ".to_string();
    //     let mut elements_dx11 = vec![];

    //     for (i, e) in elements.iter().enumerate() {
    //         write!(
    //             &mut shader_input,
    //             "{} v{i} : {}{}; ",
    //             e.hlsl_type, e.semantic_name, e.semantic_index
    //         )?;

    //         elements_dx11.push(
    //             InputElementDesc::builder()
    //                 .semantic_name(e.semantic_name)
    //                 .semantic_index(e.semantic_index)
    //                 .format(e.format)
    //                 .input_slot(e.buffer_index)
    //                 .input_slot_class(if e.is_instance_data {
    //                     d3d12::InputClassification::PerInstanceData
    //                 } else {
    //                     d3d12::InputClassification::PerVertexData
    //                 })
    //                 .instance_data_step_rate(e.is_instance_data as u32)
    //                 .build(),
    //         );
    //     }

    //     shader_input
    //         .push_str("};  float4 vs(s_vs_in input) : SV_POSITION { return float4(0, 0, 0, 0); }");

    //     let shader_blob = d3d12::fxc::compile(
    //         shader_input.as_bytes(),
    //         Some("create_vertex_declaration_inline"),
    //         &[],
    //         "vs",
    //         d3d12::fxc::ShaderTarget::Vertex,
    //     )
    //     .context("Failed to compile inline vertex declaration")?;

    //     let layout = device
    //         .create_input_layout(&elements_dx11, &shader_blob)
    //         .context("Failed to create input layout")?;

    //     Ok(layout)
    // }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderTargetBlendDesc {
    blend_enable: u32,
    logic_op_enable: u32,
    src_blend: u32,
    dest_blend: u32,
    blend_op: u32,
    src_blend_alpha: u32,
    dest_blend_alpha: u32,
    blend_op_alpha: u32,
    logic_op: u32,
    render_target_write_mask: u8,
    _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadBlendState {
    _index: u32,
    render_targets: [RenderTargetBlendDesc; 5],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RasterizerDesc {
    fill_mode: u32,
    cull_mode: u32,
    front_counter_clockwise: u32,
    _depth_bias: i32,
    _depth_bias_clamp: f32,
    _slope_scaled_depth_bias: f32,
    depth_clip_enable: u32,
    _scissor_enable: u32,
    multisample_enable: u32,
    antialiased_line_enable: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PaddedRasterizerState {
    desc: RasterizerDesc,
    _padding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShortDepthBias {
    _index: u32,
    depth_bias: i32,
    slope_scaled_depth_bias: f32,
    depth_bias_clamp: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShortDepthState {
    depth_enable: i32,
    depth_write_mask: u32,
    depth_func: u32,
    _unknown: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StencilFace {
    compare_func: u32,
    pass_op: u32,
    fail_op: u32,
    depth_fail_op: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShortStencilDesc {
    _unknown: u32,
    enabled: u32,
    read_mask: u8,
    write_mask: u8,
    _padding: u16,
    front_face: StencilFace,
    back_face: StencilFace,
}

const _: () = {
    assert!(size_of::<RenderTargetBlendDesc>() == 40);
    assert!(size_of::<QuadBlendState>() == 204);
    assert!(size_of::<RasterizerDesc>() == 40);
    assert!(size_of::<PaddedRasterizerState>() == 44);
    assert!(size_of::<ShortDepthBias>() == 16);
    assert!(size_of::<ShortDepthState>() == 16);
    assert!(size_of::<ShortStencilDesc>() == 44);
};

// D3D12 enum values below match /usr/include/dxvk/d3d12.h.
const fn map_blend_factor(value: u32) -> vk::BlendFactor {
    match value {
        1 => vk::BlendFactor::ZERO,
        2 => vk::BlendFactor::ONE,
        3 => vk::BlendFactor::SRC_COLOR,
        4 => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
        5 => vk::BlendFactor::SRC_ALPHA,
        6 => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        7 => vk::BlendFactor::DST_ALPHA,
        8 => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        9 => vk::BlendFactor::DST_COLOR,
        10 => vk::BlendFactor::ONE_MINUS_DST_COLOR,
        11 => vk::BlendFactor::SRC_ALPHA_SATURATE,
        14 => vk::BlendFactor::CONSTANT_COLOR,
        15 => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
        16 => vk::BlendFactor::SRC1_COLOR,
        17 => vk::BlendFactor::ONE_MINUS_SRC1_COLOR,
        18 => vk::BlendFactor::SRC1_ALPHA,
        19 => vk::BlendFactor::ONE_MINUS_SRC1_ALPHA,
        _ => vk::BlendFactor::ZERO,
    }
}

const fn map_blend_op(value: u32) -> vk::BlendOp {
    match value {
        1 => vk::BlendOp::ADD,
        2 => vk::BlendOp::SUBTRACT,
        3 => vk::BlendOp::REVERSE_SUBTRACT,
        4 => vk::BlendOp::MIN,
        5 => vk::BlendOp::MAX,
        _ => vk::BlendOp::ADD,
    }
}

const fn map_logic_op(value: u32) -> vk::LogicOp {
    match value {
        0 => vk::LogicOp::CLEAR,
        1 => vk::LogicOp::SET,
        2 => vk::LogicOp::COPY,
        3 => vk::LogicOp::COPY_INVERTED,
        4 => vk::LogicOp::NO_OP,
        5 => vk::LogicOp::INVERT,
        6 => vk::LogicOp::AND,
        7 => vk::LogicOp::NAND,
        8 => vk::LogicOp::OR,
        9 => vk::LogicOp::NOR,
        10 => vk::LogicOp::XOR,
        11 => vk::LogicOp::EQUIVALENT,
        12 => vk::LogicOp::AND_REVERSE,
        13 => vk::LogicOp::AND_INVERTED,
        14 => vk::LogicOp::OR_REVERSE,
        15 => vk::LogicOp::OR_INVERTED,
        _ => vk::LogicOp::COPY,
    }
}

const fn map_fill_mode(value: u32) -> vk::PolygonMode {
    match value {
        2 => vk::PolygonMode::LINE,
        _ => vk::PolygonMode::FILL,
    }
}

const fn map_cull_mode(value: u32) -> vk::CullModeFlags {
    match value {
        2 => vk::CullModeFlags::FRONT,
        3 => vk::CullModeFlags::BACK,
        _ => vk::CullModeFlags::NONE,
    }
}

const fn map_compare_op(value: u32) -> vk::CompareOp {
    match value {
        1 => vk::CompareOp::NEVER,
        2 => vk::CompareOp::LESS,
        3 => vk::CompareOp::EQUAL,
        4 => vk::CompareOp::LESS_OR_EQUAL,
        5 => vk::CompareOp::GREATER,
        6 => vk::CompareOp::NOT_EQUAL,
        7 => vk::CompareOp::GREATER_OR_EQUAL,
        8 => vk::CompareOp::ALWAYS,
        _ => vk::CompareOp::ALWAYS,
    }
}

const fn map_stencil_op(value: u32) -> vk::StencilOp {
    match value {
        1 => vk::StencilOp::KEEP,
        2 => vk::StencilOp::ZERO,
        3 => vk::StencilOp::REPLACE,
        4 => vk::StencilOp::INCREMENT_AND_CLAMP,
        5 => vk::StencilOp::DECREMENT_AND_CLAMP,
        6 => vk::StencilOp::INVERT,
        7 => vk::StencilOp::INCREMENT_AND_WRAP,
        8 => vk::StencilOp::DECREMENT_AND_WRAP,
        _ => vk::StencilOp::KEEP,
    }
}

fn map_stencil_op_state(face: StencilFace, compare_mask: u8, write_mask: u8) -> vk::StencilOpState {
    vk::StencilOpState::default()
        .fail_op(map_stencil_op(face.fail_op))
        .pass_op(map_stencil_op(face.pass_op))
        .depth_fail_op(map_stencil_op(face.depth_fail_op))
        .compare_op(map_compare_op(face.compare_func))
        .compare_mask(compare_mask as u32)
        .write_mask(write_mask as u32)
        .reference(0)
}

const DEPTH_STENCIL_COMBOS: [(usize, usize); 83] = [
    (0, 0),
    (1, 1),
    (2, 1),
    (7, 1),
    (2, 2),
    (1, 3),
    (1, 4),
    (2, 5),
    (2, 6),
    (2, 9),
    (2, 10),
    (2, 0xb),
    (2, 0xc),
    (4, 1),
    (5, 1),
    (3, 1),
    (6, 1),
    (3, 0x10),
    (8, 0x10),
    (3, 0x11),
    (3, 0x12),
    (6, 0x13),
    (6, 0x1b),
    (3, 0x13),
    (3, 0x19),
    (3, 0x1b),
    (5, 0x14),
    (2, 0x15),
    (3, 0x15),
    (3, 0x18),
    (3, 0x1a),
    (1, 0x1d),
    (1, 0x12),
    (1, 0x13),
    (9, 1),
    (10, 1),
    (3, 0x1e),
    (0xb, 0x1f),
    (1, 0x1f),
    (1, 0x20),
    (1, 0x21),
    (3, 0x21),
    (2, 0x21),
    (5, 0x20),
    (3, 0x20),
    (3, 6),
    (3, 10),
    (3, 0xb),
    (3, 0xc),
    (3, 9),
    (0xc, 0x22),
    (1, 0x23),
    (3, 0x1c),
    (6, 0x1c),
    (0xc, 0x10),
    (0xc, 0x25),
    (8, 0x24),
    (3, 0x26),
    (1, 0x26),
    (3, 0x27),
    (1, 0x27),
    (3, 0x14),
    (1, 0x14),
    (3, 0x28),
    (3, 8),
    (2, 8),
    (1, 2),
    (1, 8),
    (3, 7),
    (3, 0x17),
    (3, 0xd),
    (3, 0xe),
    (3, 0xf),
    (1, 0x29),
    (1, 0x2a),
    (1, 0x2b),
    (1, 0x2c),
    (1, 0x2d),
    (1, 0x2e),
    (1, 0x2f),
    (1, 0x30),
    (1, 0x1a),
    (2, 0x16),
];

struct TigerInputLayout {
    pub elements: &'static [TigerInputLayoutElement],
}

fn tiger_input_layout_to_vk(
    elements: &[TigerInputLayoutElement],
) -> (
    Vec<vk::VertexInputBindingDescription>,
    Vec<vk::VertexInputAttributeDescription>,
) {
    let mut attributes = vec![];

    #[derive(Clone)]
    struct Buffer {
        stride: u32,
        element_count: u32,
        is_instanced: bool,
    }

    let num_buffers = elements
        .iter()
        .map(|b| b.buffer_index + 1)
        .max()
        .unwrap_or(0);
    let mut buffers = vec![
        Buffer {
            stride: 0,
            element_count: 0,
            is_instanced: false,
        };
        num_buffers as usize
    ];

    for (location, element) in elements.iter().enumerate() {
        let buffer = &mut buffers[element.buffer_index as usize];
        attributes.push(vk::VertexInputAttributeDescription {
            location: location as u32,
            binding: element.buffer_index,
            format: element.format,
            offset: buffer.stride,
        });

        buffer.element_count += 1;
        buffer.stride += element.stride;
        if element.is_instance_data {
            buffer.is_instanced = true;
        }
    }

    let bindings = buffers
        .into_iter()
        .enumerate()
        .map(|(i, b)| vk::VertexInputBindingDescription {
            binding: i as u32,
            stride: b.stride,
            input_rate: if b.is_instanced {
                vk::VertexInputRate::INSTANCE
            } else {
                vk::VertexInputRate::VERTEX
            },
        })
        .collect();

    (bindings, attributes)
}

#[derive(Clone)]
struct TigerInputLayoutElement {
    pub _hlsl_type: &'static str,
    pub format: vk::Format,
    pub stride: u32,
    pub semantic_name: &'static str,
    pub semantic_index: u32,
    pub buffer_index: u32,
    pub is_instance_data: bool,
}

// impl From<TigerInputLayoutElement> for d3d12::InputElementDesc {
//     fn from(value: TigerInputLayoutElement) -> Self {
//         Self::builder()
//             .semantic_name(value.semantic_name)
//             .semantic_index(value.semantic_index)
//             .format(value.format)
//             .input_slot(value.buffer_index)
//             .input_slot_class(if value.is_instance_data {
//                 d3d12::InputClassification::PerInstanceData
//             } else {
//                 d3d12::InputClassification::PerVertexData
//             })
//             .instance_data_step_rate(value.is_instance_data as u32)
//             .build()
//     }
// }

struct InputElementFormat {
    hlsl_type: &'static str,
    stride: u32,
    format: vk::Format,
}

//region Built-in input layouts
const INPUT_SEMANTICS: [&str; 9] = [
    "POSITION",
    "BLENDWEIGHT",
    "BLENDINDICES",
    "NORMAL",
    "PSIZE",
    "TEXCOORD",
    "TANGENT",
    "BINORMAL",
    "COLOR",
];

#[rustfmt::skip]
const INPUT_FORMATS: [InputElementFormat; 34] = [
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 0
    InputElementFormat { hlsl_type: "float", stride: 4, format: vk::Format::R32_SFLOAT }, // 1
    InputElementFormat { hlsl_type: "float2", stride: 8, format: vk::Format::R32G32_SFLOAT }, // 2
    InputElementFormat { hlsl_type: "float3", stride: 12, format: vk::Format::R32G32B32_SFLOAT }, // 3
    InputElementFormat { hlsl_type: "float4", stride: 16, format: vk::Format::R32G32B32A32_SFLOAT }, // 4
    InputElementFormat { hlsl_type: "float4", stride: 4, format: vk::Format::R8G8B8A8_UNORM }, // 5
    InputElementFormat { hlsl_type: "uint4", stride: 4, format: vk::Format::R8G8B8A8_UINT }, // 6
    InputElementFormat { hlsl_type: "int2", stride: 4, format: vk::Format::R16G16_SINT }, // 7
    InputElementFormat { hlsl_type: "int4", stride: 8, format: vk::Format::R16G16B16A16_SINT }, // 8
    InputElementFormat { hlsl_type: "uint4", stride: 8, format: vk::Format::R16G16B16A16_UINT }, // 9
    InputElementFormat { hlsl_type: "float2", stride: 4, format: vk::Format::R16G16_SNORM }, // 10
    InputElementFormat { hlsl_type: "float4", stride: 8, format: vk::Format::R16G16B16A16_SNORM }, // 11
    InputElementFormat { hlsl_type: "float2", stride: 4, format: vk::Format::R16G16_SFLOAT }, // 12
    InputElementFormat { hlsl_type: "float4", stride: 8, format: vk::Format::R16G16B16A16_SFLOAT }, // 13
    InputElementFormat { hlsl_type: "int4", stride: 4, format: vk::Format::R8G8B8A8_SINT }, // 14
    InputElementFormat { hlsl_type: "float4", stride: 4, format: vk::Format::R8G8B8A8_SNORM }, // 15
    InputElementFormat { hlsl_type: "uint4", stride: 4, format: vk::Format::A2R10G10B10_UINT_PACK32 }, // 16
    InputElementFormat { hlsl_type: "float4", stride: 4, format: vk::Format::A2R10G10B10_UNORM_PACK32 }, // 17
    InputElementFormat { hlsl_type: "int", stride: 4, format: vk::Format::R32_SINT }, // 18
    InputElementFormat { hlsl_type: "int2", stride: 8, format: vk::Format::R32G32_SINT }, // 19
    InputElementFormat { hlsl_type: "int4", stride: 16, format: vk::Format::R32G32B32A32_SINT }, // 20
    InputElementFormat { hlsl_type: "int", stride: 4, format: vk::Format::R32_UINT }, // 21
    InputElementFormat { hlsl_type: "int2", stride: 8, format: vk::Format::R32G32_UINT }, // 22
    InputElementFormat { hlsl_type: "int4", stride: 16, format: vk::Format::R32G32B32A32_UINT }, // 23
    InputElementFormat { hlsl_type: "int", stride: 2, format: vk::Format::R16_SINT }, // 24
    InputElementFormat { hlsl_type: "float", stride: 1, format: vk::Format::UNDEFINED }, // 25
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 26
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 27
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 28
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 29
    InputElementFormat { hlsl_type: "", stride: 0, format: vk::Format::UNDEFINED, }, // 30
    InputElementFormat { hlsl_type: "float4", stride: 4, format: vk::Format::R8G8B8A8_UNORM }, // 31
    InputElementFormat { hlsl_type: "float3", stride: 4, format: vk::Format::B10G11R11_UFLOAT_PACK32 }, // 32
    InputElementFormat { hlsl_type: "float4", stride: 8, format: vk::Format::R16G16B16A16_SNORM }, // 33
];

const BASE_INPUT_LAYOUTS: [TigerInputLayout; 7] = [
    // Layout 0
    TigerInputLayout {
        elements: &[TigerInputLayoutElement {
            _hlsl_type: "float3",
            format: vk::Format::R32G32B32_SFLOAT,
            stride: 12,
            semantic_name: "POSITION",
            semantic_index: 0,
            buffer_index: 0,
            is_instance_data: false,
        }],
    },
    // Layout 1
    TigerInputLayout {
        elements: &[TigerInputLayoutElement {
            _hlsl_type: "float3",
            format: vk::Format::R32G32B32_SFLOAT,
            stride: 12,
            semantic_name: "POSITION",
            semantic_index: 0,
            buffer_index: 0,
            is_instance_data: false,
        }],
    },
    // Layout 2
    TigerInputLayout {
        elements: &[
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "POSITION",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "TEXCOORD",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float4",
                format: vk::Format::R8G8B8A8_UNORM,
                stride: 4,
                semantic_name: "COLOR",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
        ],
    },
    // Layout 3
    TigerInputLayout {
        elements: &[
            TigerInputLayoutElement {
                _hlsl_type: "float3",
                format: vk::Format::R32G32B32_SFLOAT,
                stride: 12,
                semantic_name: "POSITION",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "TEXCOORD",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float4",
                format: vk::Format::R8G8B8A8_UNORM,
                stride: 4,
                semantic_name: "COLOR",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
        ],
    },
    // Layout 4
    TigerInputLayout {
        elements: &[
            TigerInputLayoutElement {
                _hlsl_type: "float3",
                format: vk::Format::R32G32B32_SFLOAT,
                stride: 12,
                semantic_name: "POSITION",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float4",
                format: vk::Format::R8G8B8A8_UNORM,
                stride: 4,
                semantic_name: "COLOR",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
        ],
    },
    // Layout 5
    TigerInputLayout {
        elements: &[
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "POSITION",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "TEXCOORD",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
        ],
    },
    // Layout 6
    TigerInputLayout {
        elements: &[
            TigerInputLayoutElement {
                _hlsl_type: "float3",
                format: vk::Format::R32G32B32_SFLOAT,
                stride: 12,
                semantic_name: "POSITION",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float3",
                format: vk::Format::R32G32B32_SFLOAT,
                stride: 12,
                semantic_name: "NORMAL",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float4",
                format: vk::Format::R32G32B32A32_SFLOAT,
                stride: 16,
                semantic_name: "TANGENT",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
            TigerInputLayoutElement {
                _hlsl_type: "float2",
                format: vk::Format::R32G32_SFLOAT,
                stride: 8,
                semantic_name: "TEXCOORD",
                semantic_index: 0,
                buffer_index: 0,
                is_instance_data: false,
            },
        ],
    },
];
//endregion
