use std::collections::hash_map::Entry;

use ahash::HashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::tfx::{FixedFunctionState, ShaderStage};
use tiger_pkg::{TagHash, package_manager};

use crate::gpu::{command_list::DepthMode, global_state::RenderStates};

const SETS_PER_STAGE: u32 = 4;
pub const SET_BASE_VERTEX: u32 = 0;
pub const SET_BASE_PIXEL: u32 = SETS_PER_STAGE;
pub const SET_BASE_GEOMETRY: u32 = SETS_PER_STAGE * 2;
pub const SET_BASE_HULL: u32 = SETS_PER_STAGE * 3;
pub const SET_BASE_DOMAIN: u32 = SETS_PER_STAGE * 4;
pub const SET_BASE_COMPUTE: u32 = SETS_PER_STAGE * 5;

const TOTAL_SETS: usize = 8;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PipelineKey {
    pub vertex_shader: TagHash,
    pub pixel_shader: TagHash,
    pub depth_mode: DepthMode,
    pub fixed_function_state: FixedFunctionState,
    pub topology: vk::PrimitiveTopology,
    pub input_layout: u8,
    pub color_formats: Vec<vk::Format>,
    pub depth_format: Option<vk::Format>,
}

#[derive(Clone)]
pub struct ShaderInfo {
    pub module: vk::ShaderModule,
    pub slots: dxil2spirv::SlotUsage,
}

impl ShaderInfo {
    fn empty() -> Self {
        Self {
            module: vk::ShaderModule::null(),
            slots: dxil2spirv::SlotUsage::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TechniqueLayout {
    pub pipeline_layout: vk::PipelineLayout,
    pub set_layouts: [vk::DescriptorSetLayout; TOTAL_SETS],
}

pub struct PipelineCache {
    render_states: RenderStates,
    bytecode_cache: HashMap<(TagHash, u32), ShaderInfo>,
    layout_cache: HashMap<(TagHash, TagHash), TechniqueLayout>,
    storage: HashMap<PipelineKey, vk::Pipeline>,

    empty_layout: vk::DescriptorSetLayout,

    device: ash::Device,
}

impl PipelineCache {
    pub fn new(device: ash::Device) -> Self {
        let empty_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default()
                    .flags(vk::DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT),
                None,
            )
        }
        .expect("Failed to create empty descriptor set layout");

        Self {
            render_states: RenderStates::new(&device).expect("Failed to load render states"),
            bytecode_cache: HashMap::default(),
            layout_cache: HashMap::default(),
            storage: HashMap::default(),
            empty_layout,
            device,
        }
    }

    pub fn shader_info(&mut self, hash: TagHash, stage: ShaderStage) -> anyhow::Result<ShaderInfo> {
        self.get_or_load_bytecode(hash, stage)
    }

    #[profiling::function]
    pub fn get_or_create_layout(
        &mut self,
        vertex_shader: TagHash,
        pixel_shader: TagHash,
    ) -> anyhow::Result<TechniqueLayout> {
        if let Some(layout) = self.layout_cache.get(&(vertex_shader, pixel_shader)) {
            return Ok(*layout);
        }

        let vs_info = self.get_or_load_bytecode(vertex_shader, ShaderStage::Vertex)?;
        let ps_info = self.get_or_load_bytecode(pixel_shader, ShaderStage::Pixel)?;

        let vs_sets = self.build_stage_set_layouts(&vs_info.slots, ShaderStage::Vertex)?;
        let ps_sets = self.build_stage_set_layouts(&ps_info.slots, ShaderStage::Pixel)?;

        let set_layouts = [
            vs_sets[0], vs_sets[1], vs_sets[2], vs_sets[3], ps_sets[0], ps_sets[1], ps_sets[2],
            ps_sets[3],
        ];

        let pipeline_layout = unsafe {
            self.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
                None,
            )
        }
        .context("failed to create pipeline layout")?;

        let layout = TechniqueLayout {
            pipeline_layout,
            set_layouts,
        };

        self.layout_cache
            .insert((vertex_shader, pixel_shader), layout);

        Ok(layout)
    }

    fn build_stage_set_layouts(
        &self,
        slots: &dxil2spirv::SlotUsage,
        stage: ShaderStage,
    ) -> anyhow::Result<[vk::DescriptorSetLayout; 4]> {
        let stage_flags = stage_flags(stage);

        let build = |mask: u32, descriptor_type: &dyn Fn(u32) -> vk::DescriptorType| {
            if mask == 0 {
                return Ok(self.empty_layout);
            }

            let bindings = dxil2spirv::SlotUsage::iter_mask(mask)
                .map(|slot| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(slot)
                        .descriptor_type(descriptor_type(slot))
                        .descriptor_count(1)
                        .stage_flags(stage_flags)
                })
                .collect::<Vec<_>>();

            unsafe {
                self.device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .flags(vk::DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT)
                        .bindings(&bindings),
                    None,
                )
            }
            .context("failed to create descriptor set layout")
        };

        let cbv = build(slots.cbv, &|_| vk::DescriptorType::UNIFORM_BUFFER)?;

        let srv = build(slots.srv, &|slot| match slots.srv_types[slot as usize] {
            dxil2spirv::SlotType::UniformTexelBuffer => vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
            dxil2spirv::SlotType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
            dxil2spirv::SlotType::StorageTexelBuffer => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
            dxil2spirv::SlotType::SampledImage
            | dxil2spirv::SlotType::StorageImage
            | dxil2spirv::SlotType::UniformBuffer
            | dxil2spirv::SlotType::Sampler
            | dxil2spirv::SlotType::Unknown => vk::DescriptorType::SAMPLED_IMAGE,
        })?;

        let sampler = build(slots.sampler, &|_| vk::DescriptorType::SAMPLER)?;

        let uav = build(slots.uav, &|slot| match slots.uav_types[slot as usize] {
            dxil2spirv::SlotType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
            dxil2spirv::SlotType::StorageTexelBuffer => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
            _ => vk::DescriptorType::STORAGE_IMAGE,
        })?;

        Ok([cbv, srv, sampler, uav])
    }

    fn get_or_create_pipeline(
        &mut self,
        key: &PipelineKey,
        layout: TechniqueLayout,
        vs_info: &ShaderInfo,
        ps_info: &ShaderInfo,
    ) -> anyhow::Result<vk::Pipeline> {
        let (vertex_binding_descs, vertex_attribute_descs) = self
            .render_states
            .input_layouts
            .get(key.input_layout as usize)
            .with_context(|| format!("invalid input layout {}", key.input_layout))?;

        let mut stages = vec![
            vk::PipelineShaderStageCreateInfo::default()
                .name(c"main")
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs_info.module),
        ];
        if ps_info.module != vk::ShaderModule::null() {
            stages.push(
                vk::PipelineShaderStageCreateInfo::default()
                    .name(c"main")
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(ps_info.module),
            );
        }

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // TODO(cohae): parse and translate rasterizer/depth/blend states
        let rasterizer_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(key.depth_format.is_some())
            .depth_write_enable(key.depth_format.is_some())
            .depth_compare_op(match key.depth_mode {
                DepthMode::Reverse => vk::CompareOp::GREATER_OR_EQUAL,
                DepthMode::Forward => vk::CompareOp::LESS_OR_EQUAL,
            });

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

        let attachments = key
            .color_formats
            .iter()
            .map(|_| {
                vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA)
                    .blend_enable(false)
            })
            .collect::<Vec<_>>();

        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
            vk::DynamicState::PRIMITIVE_TOPOLOGY,
        ];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let mut pipeline_rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&key.color_formats)
            .depth_attachment_format(key.depth_format.unwrap_or(vk::Format::UNDEFINED));

        let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
            .flags(vk::PipelineCreateFlags::DESCRIPTOR_BUFFER_EXT)
            .layout(layout.pipeline_layout)
            .stages(&stages)
            .multisample_state(&multisample)
            .rasterization_state(&rasterizer_state)
            .depth_stencil_state(&depth_stencil_state)
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
                .map_err(|(_, e)| e)?[0]
        };

        // pipeline.set_debug_name(format!("vs_{}_ps_{}", key.vertex_shader, key.pixel_shader));
        Ok(pipeline)
    }

    #[profiling::function]
    pub fn get_or_create(&mut self, key: &PipelineKey) -> anyhow::Result<vk::Pipeline> {
        if let Some(pipeline) = self.storage.get(key) {
            return Ok(*pipeline);
        }

        let layout = self.get_or_create_layout(key.vertex_shader, key.pixel_shader)?;
        let vs_info = self.get_or_load_bytecode(key.vertex_shader, ShaderStage::Vertex)?;
        let ps_info = self.get_or_load_bytecode(key.pixel_shader, ShaderStage::Pixel)?;

        let pipeline = self.get_or_create_pipeline(key, layout, &vs_info, &ps_info)?;

        self.storage.insert(key.clone(), pipeline);
        Ok(pipeline)
    }

    #[profiling::function]
    pub fn get_or_load_bytecode(
        &mut self,
        hash: TagHash,
        stage: ShaderStage,
    ) -> anyhow::Result<ShaderInfo> {
        if hash.is_none() {
            return Ok(ShaderInfo::empty());
        }

        let set_base = set_base(stage);
        match self.bytecode_cache.entry((hash, set_base)) {
            Entry::Occupied(entry) => Ok(entry.into_mut().clone()),
            Entry::Vacant(entry) => {
                let tag_entry = package_manager()
                    .get_entry(hash)
                    .context("entry not found")?;
                let data = package_manager()
                    .read_tag(tag_entry.reference)
                    .context("failed to read shader bytecode")?;

                let (spirv, slots) =
                    dxil2spirv::translate(&data, set_base).map_err(|e| anyhow::anyhow!(e))?;

                let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv);
                let module = unsafe { self.device.create_shader_module(&create_info, None)? };

                Ok(entry.insert(ShaderInfo { module, slots }).clone())
            }
        }
    }
}

const fn set_base(stage: ShaderStage) -> u32 {
    match stage {
        ShaderStage::Vertex => SET_BASE_VERTEX,
        ShaderStage::Pixel => SET_BASE_PIXEL,
        ShaderStage::Geometry => SET_BASE_GEOMETRY,
        ShaderStage::Hull => SET_BASE_HULL,
        ShaderStage::Domain => SET_BASE_DOMAIN,
        ShaderStage::Compute => SET_BASE_COMPUTE,
    }
}

const fn stage_flags(stage: ShaderStage) -> vk::ShaderStageFlags {
    match stage {
        ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
        ShaderStage::Pixel => vk::ShaderStageFlags::FRAGMENT,
        ShaderStage::Geometry => vk::ShaderStageFlags::GEOMETRY,
        ShaderStage::Hull => vk::ShaderStageFlags::TESSELLATION_CONTROL,
        ShaderStage::Domain => vk::ShaderStageFlags::TESSELLATION_EVALUATION,
        ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
    }
}
