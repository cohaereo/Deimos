use std::collections::hash_map::Entry;

use ahash::HashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::FixedFunctionState;
use itertools::Itertools;
use tiger_pkg::{TagHash, package_manager};

use crate::gpu::{command_list::DepthMode, global_state::RenderStates};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PipelineKey {
    pub vertex_shader: TagHash,
    pub pixel_shader: TagHash,
    pub depth_mode: DepthMode,
    pub fixed_function_state: FixedFunctionState,
    pub topology: vk::PrimitiveTopology,
    pub input_layout: u8,
}

pub struct PipelineCache {
    render_states: RenderStates,
    bytecode_cache: HashMap<TagHash, (vk::ShaderModule, dxil2spirv::SlotUsage)>,
    storage: HashMap<PipelineKey, vk::Pipeline>,

    device: ash::Device,
}

impl PipelineCache {
    pub fn new(device: ash::Device) -> Self {
        Self {
            render_states: RenderStates::new(&device).expect("Failed to load render states"),
            bytecode_cache: HashMap::default(),
            storage: HashMap::default(),
            device,
        }
    }

    #[profiling::function]
    pub fn get_or_create(
        &mut self,
        key: PipelineKey,
        pipeline_layout: vk::PipelineLayout,
        rtv_formats: &[vk::Format],
        dsv_format: Option<vk::Format>,
    ) -> anyhow::Result<vk::Pipeline> {
        if self.storage.contains_key(&key) {
            return Ok(self
                .storage
                .get(&key)
                .copied()
                .expect("unreachable: just checked"));
        }

        let (vs_module, _) = self.get_or_load_bytecode(key.vertex_shader)?;
        let (ps_module, _) = self.get_or_load_bytecode(key.pixel_shader)?;

        let (vertex_binding_descs, vertex_attribute_descs) = self
            .render_states
            .input_layouts
            .get(key.input_layout as usize)
            .with_context(|| format!("invalid input layout {}", key.input_layout))?;

        // let blend_state_index = key.fixed_function_state.blend_state().unwrap_or(0);
        // let blend_state = self
        //     .render_states
        //     .blend_states
        //     .get(blend_state_index)
        //     .context("invalid blend state")?;

        // let depth_stencil_state_index = key.fixed_function_state.depth_stencil_state().unwrap_or(0);
        // let (depth_stencil_state_reverse, depth_stencil_state_forward) = self
        //     .render_states
        //     .depth_stencil_states
        //     .get(depth_stencil_state_index)
        //     .context("invalid depth stencil state")?;
        // let depth_stencil_state = *match key.depth_mode {
        //     DepthMode::Forward => depth_stencil_state_forward,
        //     DepthMode::Reverse => depth_stencil_state_reverse,
        // };

        // let rasterizer_state_index = key.fixed_function_state.rasterizer_state().unwrap_or(0);
        // let depth_bias_index = key.fixed_function_state.depth_bias_state().unwrap_or(0);
        // let rasterizer_state = *self
        //     .render_states
        //     .rasterizer_states
        //     .get(rasterizer_state_index)
        //     .context("invalid rasterizer state")?
        //     .get(depth_bias_index)
        //     .context("invalid depth bias state")?;

        // // let depth_state = vk_DEPTH_STENCIL_DESC {
        // //     DepthEnable: true.into(),
        // //     DepthWriteMask: vk::vk_DEPTH_WRITE_MASK_ALL,
        // //     DepthFunc: vk::vk_COMPARISON_FUNC_GREATER_EQUAL,
        // //     StencilEnable: false.into(),
        // //     ..Default::default()
        // // };

        // let mut desc = GraphicsPipelineStateDesc::new(root_signature)
        //     .with_vs(&vs)
        //     .with_input_layout(input_layout.as_slice())
        //     .with_primitive_topology(vk::PrimitiveTopology2::Triangle)
        //     .with_blend_state(blend_state.clone())
        //     .with_dsv_format(dsv_format.unwrap_or_default())
        //     .with_rasterizer_state(rasterizer_state)
        //     .with_depth_stencil_state(depth_stencil_state);

        // if !ps.is_empty() {
        //     desc = desc.with_rtv_formats(rtv_formats).with_ps(&ps);
        // }

        // let pipeline = self
        //     .device
        //     .create_graphics_pipeline_state(&desc)
        //     .context("create_graphics_pipeline_state")?;

        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .name(c"main")
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs_module),
            vk::PipelineShaderStageCreateInfo::default()
                .name(c"main")
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(ps_module),
        ];

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let rasterizer_state = vk::PipelineRasterizationStateCreateInfo::default().line_width(1.0);

        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(vertex_binding_descs)
            .vertex_attribute_descriptions(vertex_attribute_descs);

        let input_assembly =
            vk::PipelineInputAssemblyStateCreateInfo::default().topology(key.topology);

        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        let attachments = rtv_formats
            .iter()
            .map(|_| {
                vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA)
                    .blend_enable(false)
            })
            .collect_vec();

        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

        let mut pipeline_rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(rtv_formats)
            .depth_attachment_format(dsv_format.unwrap_or(vk::Format::UNDEFINED));

        let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
            .layout(pipeline_layout)
            .stages(&stages)
            .multisample_state(&multisample)
            .rasterization_state(&rasterizer_state)
            .viewport_state(&viewport_state)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly)
            .color_blend_state(&color_blend_state)
            .subpass(0)
            .dynamic_state(&dynamic_state)
            .push_next(&mut pipeline_rendering_info);

        let pipeline = unsafe {
            self.device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    std::slice::from_ref(&pipeline_create_info),
                    None,
                )
                .unwrap()[0]
        };

        // pipeline.set_debug_name(format!("vs_{}_ps_{}", key.vertex_shader, key.pixel_shader));

        self.storage.insert(key.clone(), pipeline);
        Ok(pipeline)
    }

    #[profiling::function]
    pub fn get_or_load_bytecode(
        &mut self,
        hash: TagHash,
    ) -> anyhow::Result<(vk::ShaderModule, dxil2spirv::SlotUsage)> {
        if hash.is_none() {
            return Ok((vk::ShaderModule::null(), dxil2spirv::SlotUsage::default()));
        }

        match self.bytecode_cache.entry(hash) {
            Entry::Occupied(entry) => Ok(entry.into_mut().clone()),
            Entry::Vacant(entry) => {
                let tag_entry = package_manager()
                    .get_entry(hash)
                    .context("entry not found")?;
                let data = package_manager()
                    .read_tag(tag_entry.reference)
                    .context("failed to read shader bytecode")?;

                let (spirv, slots) = match dxil2spirv::translate(&data) {
                    Ok(result) => result,
                    Err(e) => return Err(anyhow::anyhow!(e)),
                };

                let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv);
                let module = unsafe { self.device.create_shader_module(&create_info, None)? };

                Ok(entry.insert((module, slots)).clone())
            }
        }
    }
}
