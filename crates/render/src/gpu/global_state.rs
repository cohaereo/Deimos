use anyhow::Context;
use ash::vk;
use deimos_data::tfx::render_globals::SRenderGlobals;
use tiger_parse::PackageManagerExt;
use tiger_pkg::package_manager;

use crate::util::byteutil;

const DEPTH_STENCIL_COMBO_COUNT: usize = DEPTH_STENCIL_COMBOS.len();
pub struct RenderStates {
    pub input_layouts: Vec<(
        Vec<vk::VertexInputBindingDescription>,
        Vec<vk::VertexInputAttributeDescription>,
    )>,
    // pub blend_states: Vec<d3d12::BlendDesc>,
    // pub rasterizer_states: [[d3d12::D3D12_RASTERIZER_DESC; 9]; 9],
    // // /// 1st state is for reverse Z, 2nd state is for regular Z (commonly used for shadow maps)
    // pub depth_stencil_states:
    //     [(D3D12_DEPTH_STENCIL_DESC, D3D12_DEPTH_STENCIL_DESC); DEPTH_STENCIL_COMBO_COUNT],
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

        // let mut blend_states = vec![];

        // let blend_state_descs: &[QuadBlendState] =
        //     unsafe { byteutil::bytes_as_slice(Self::BLEND_STATE_DESC_DATA) };
        // // let chroma = ChromaConfig {
        // //     inline_struct: chroma_dbg::InlineThreshold::Always,
        // //     ..Default::default()
        // // };
        // for desc in blend_state_descs.iter() {
        //     // println!("{}", chroma.format(desc));

        //     let render_targets = [
        //         desc.render_targets[0].clone(),
        //         desc.render_targets[1].clone(),
        //         desc.render_targets[2].clone(),
        //         desc.render_targets[3].clone(),
        //         desc.render_targets[4].clone(),
        //         // Extend to 8 render targets
        //         desc.render_targets[4].clone(),
        //         desc.render_targets[4].clone(),
        //         desc.render_targets[4].clone(),
        //     ];

        //     let state = d3d12::BlendDesc::builder()
        //         .alpha_to_coverage_enable(false)
        //         .independent_blend_enable(true)
        //         .render_target(render_targets)
        //         .build();
        //     blend_states.push(state);
        // }

        // let depth_biases: &[ShortDepthBias] =
        //     unsafe { byteutil::bytes_as_slice(Self::DEPTH_BIAS_DATA) };
        // let rasterizer_states: &[PaddedRasterizerState] =
        //     unsafe { byteutil::bytes_as_slice(Self::RASTERIZER_STATE_DATA) };
        // assert_eq!(rasterizer_states.len(), 9);
        // assert_eq!(depth_biases.len(), 9);

        // let rasterizer_states = std::array::from_fn(|bi| {
        //     std::array::from_fn(|ri| {
        //         let desc11 = &rasterizer_states[ri].desc;
        //         let ShortDepthBias(_, depth_bias, slope_scaled_depth_bias, depth_bias_clamp) =
        //             depth_biases[bi];

        //         D3D12_RASTERIZER_DESC {
        //             FillMode: desc11.fill_mode,
        //             CullMode: desc11.cull_mode,
        //             FrontCounterClockwise: desc11.front_counter_clockwise,
        //             DepthBias: depth_bias,
        //             DepthBiasClamp: depth_bias_clamp,
        //             SlopeScaledDepthBias: slope_scaled_depth_bias,
        //             DepthClipEnable: desc11.depth_clip_enable,
        //             MultisampleEnable: desc11.multisample_enable,
        //             AntialiasedLineEnable: desc11.antialiased_line_enable,
        //             ForcedSampleCount: 0,
        //             ConservativeRaster: d3d12::D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        //         }
        //     })
        // });

        // let depth_states: &[ShortDepthState] =
        //     unsafe { byteutil::bytes_as_slice(Self::DEPTH_STATE_DATA) };
        // let stencil_states: &[ShortStencilDesc] =
        //     unsafe { byteutil::bytes_as_slice(Self::STENCIL_STATE_DATA) };

        // // warn!("!!! STENCIL TESTING IS DISABLED");
        // let depth_stencil_states = std::array::from_fn(|i| {
        //     let (depth_index, stencil_index) = DEPTH_STENCIL_COMBOS[i];

        //     let depth = depth_states[depth_index].clone();
        //     let stencil = stencil_states[stencil_index].clone();
        //     let reversed_depth_func = Self::REVERSED_DEPTH_FUNCS[depth_index];

        //     let desc_reverse = d3d12::D3D12_DEPTH_STENCIL_DESC {
        //         DepthEnable: BOOL(depth.0),
        //         DepthWriteMask: depth.1,
        //         DepthFunc: depth.2.into(),
        //         StencilEnable: false.into(), //stencil.enabled,
        //         StencilReadMask: stencil.read_mask,
        //         StencilWriteMask: stencil.write_mask,
        //         FrontFace: d3d12::D3D12_DEPTH_STENCILOP_DESC {
        //             StencilFailOp: stencil.front_face.2,
        //             StencilDepthFailOp: stencil.front_face.3,
        //             StencilPassOp: stencil.front_face.1,
        //             StencilFunc: stencil.front_face.0.into(),
        //         },
        //         BackFace: d3d12::D3D12_DEPTH_STENCILOP_DESC {
        //             StencilFailOp: stencil.back_face.2,
        //             StencilDepthFailOp: stencil.back_face.3,
        //             StencilPassOp: stencil.back_face.1,
        //             StencilFunc: stencil.back_face.0.into(),
        //         },
        //     };

        //     let desc_forward = D3D12_DEPTH_STENCIL_DESC {
        //         DepthFunc: reversed_depth_func.into(),
        //         ..desc_reverse
        //     };

        //     (desc_reverse, desc_forward)
        // });

        Ok(Self {
            input_layouts,
            // blend_states,
            // rasterizer_states,
            // depth_stencil_states,
        })
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

// /// Regular d3d11 blend state, but with only 4 render targets
// #[repr(C)]
// #[derive(Debug)]
// pub struct QuadBlendState {
//     pub index: u32,
//     pub render_targets: [d3d12::RenderTargetBlendDesc; 5],
// }

// #[repr(C)]
// #[derive(Debug)]
// pub struct PaddedRasterizerState {
//     pub desc: D3d11RasterizerDesc,
//     _pad: u32,
// }

// #[repr(C)]
// #[derive(Debug, Clone)]
// pub struct ShortDepthBias(u32, i32, f32, f32);

// #[repr(C)]
// #[derive(Debug, Clone)]
// pub struct ShortDepthState(i32, D3D12_DEPTH_WRITE_MASK, d3d12::ComparisonFunc, u32);

// #[repr(C)]
// #[derive(Debug, Clone)]
// pub struct ShortStencilDesc {
//     pub unk0: u32,
//     pub enabled: BOOL,
//     pub read_mask: u8,
//     pub write_mask: u8,
//     _pad0: u16,
//     pub front_face: (
//         d3d12::ComparisonFunc,
//         D3D12_STENCIL_OP,
//         D3D12_STENCIL_OP,
//         D3D12_STENCIL_OP,
//     ),
//     pub back_face: (
//         d3d12::ComparisonFunc,
//         D3D12_STENCIL_OP,
//         D3D12_STENCIL_OP,
//         D3D12_STENCIL_OP,
//     ),
// }

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

    for element in elements {
        attributes.push(vk::VertexInputAttributeDescription {
            location: element.buffer_index,
            binding: buffers[element.buffer_index as usize].element_count,
            format: element.format,
            offset: buffers[element.buffer_index as usize].stride,
        });

        buffers[element.buffer_index as usize].element_count += 1;
        buffers[element.buffer_index as usize].stride += element.stride;
        if element.is_instance_data {
            buffers[element.buffer_index as usize].is_instanced = true;
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

// #[repr(C)]
// #[derive(Debug, Clone)]
// pub struct D3d11RasterizerDesc {
//     pub fill_mode: D3D12_FILL_MODE,
//     pub cull_mode: D3D12_CULL_MODE,
//     pub front_counter_clockwise: BOOL,
//     pub depth_bias: i32,
//     pub depth_bias_clamp: f32,
//     pub slope_scaled_depth_bias: f32,
//     pub depth_clip_enable: BOOL,
//     pub scissor_enable: BOOL,
//     pub multisample_enable: BOOL,
//     pub antialiased_line_enable: BOOL,
// }
