use std::{borrow::Cow, collections::hash_map::Entry, ffi::CString, sync::Arc};

use ahash::HashMap;
use anyhow::Context;
#[cfg(debug_assertions)]
use ash::ext::debug_utils;
use ash::{ext::descriptor_buffer, vk};
use deimos_data::tfx::{FixedFunctionState, ShaderStage};
use smallvec::SmallVec;
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
    pub color_formats: SmallVec<[vk::Format; 8]>,
    pub depth_format: Option<vk::Format>,
}

#[derive(Clone)]
pub struct ShaderInfo {
    pub module_tag: TagHash,
    pub module: vk::ShaderModule,
    pub slots: dxil2spirv::SlotUsage,
}

impl ShaderInfo {
    fn empty() -> Self {
        Self {
            module_tag: TagHash::default(),
            module: vk::ShaderModule::null(),
            slots: dxil2spirv::SlotUsage::default(),
        }
    }
}

#[derive(Clone)]
pub struct TechniqueLayout {
    pub pipeline_layout: vk::PipelineLayout,
    pub set_layouts: [vk::DescriptorSetLayout; TOTAL_SETS],
    pub set_sizes: [u64; TOTAL_SETS],
    pub binding_offsets: Arc<[[u64; 32]; TOTAL_SETS]>,
}

pub struct PipelineCache {
    render_states: RenderStates,
    bytecode_cache: HashMap<(TagHash, u32), ShaderInfo>,
    builtin_shaders: HashMap<TagHash, &'static [u8]>,
    layout_cache: HashMap<(TagHash, TagHash), TechniqueLayout>,
    storage: HashMap<PipelineKey, vk::Pipeline>,

    empty_layout: vk::DescriptorSetLayout,

    device: ash::Device,
    descriptor_buffer: descriptor_buffer::Device,
    #[cfg(debug_assertions)]
    debug_loader: debug_utils::Device,
}

impl PipelineCache {
    pub fn new(
        instance: &ash::Instance,
        device: ash::Device,
        descriptor_buffer: descriptor_buffer::Device,
    ) -> Self {
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
            builtin_shaders: HashMap::default(),
            layout_cache: HashMap::default(),
            storage: HashMap::default(),
            empty_layout,
            #[cfg(debug_assertions)]
            debug_loader: debug_utils::Device::new(instance, &device),
            device,
            descriptor_buffer,
        }
    }

    pub fn shader_info(&mut self, hash: TagHash, stage: ShaderStage) -> anyhow::Result<ShaderInfo> {
        self.get_or_load_bytecode(hash, stage)
    }

    pub fn register_builtin_shader_module(&mut self, hash: TagHash, data: &'static [u8]) {
        self.builtin_shaders.insert(hash, data);
    }

    #[profiling::function]
    pub fn get_or_create_layout(
        &mut self,
        vertex_shader: TagHash,
        pixel_shader: TagHash,
    ) -> anyhow::Result<TechniqueLayout> {
        if let Some(layout) = self.layout_cache.get(&(vertex_shader, pixel_shader)) {
            return Ok(layout.clone());
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

        let mut set_sizes = [0u64; TOTAL_SETS];
        let mut binding_offsets = [[0u64; 32]; TOTAL_SETS];
        for (set_index, set_layout) in set_layouts.iter().enumerate() {
            let size = unsafe {
                self.descriptor_buffer
                    .get_descriptor_set_layout_size(*set_layout)
            };
            set_sizes[set_index] = size;
            if size == 0 {
                continue;
            }

            let slots = if set_index < SETS_PER_STAGE as usize {
                &vs_info.slots
            } else {
                &ps_info.slots
            };
            let mask = match (set_index % SETS_PER_STAGE as usize) as u32 {
                dxil2spirv::SET_CBV => slots.cbv,
                dxil2spirv::SET_SRV => slots.srv,
                dxil2spirv::SET_SAMPLER => slots.sampler,
                _ => slots.uav,
            };

            for slot in dxil2spirv::SlotUsage::iter_mask(mask) {
                binding_offsets[set_index][slot as usize] = unsafe {
                    self.descriptor_buffer
                        .get_descriptor_set_layout_binding_offset(*set_layout, slot)
                };
            }
        }

        let layout = TechniqueLayout {
            pipeline_layout,
            set_layouts,
            set_sizes,
            binding_offsets: Arc::new(binding_offsets),
        };

        self.layout_cache
            .insert((vertex_shader, pixel_shader), layout.clone());

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
        layout: &TechniqueLayout,
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

        let fixed_function_state = key.fixed_function_state;
        let blend_index = fixed_function_state.blend_state().unwrap_or(0);
        let rasterizer_index = fixed_function_state.rasterizer_state().unwrap_or(0);
        let depth_bias_index = fixed_function_state.depth_bias_state().unwrap_or(0);
        let depth_stencil_index = fixed_function_state.depth_stencil_state().unwrap_or(0);

        let blend_state = self
            .render_states
            .blend_states
            .get(blend_index)
            .with_context(|| format!("invalid blend state index {blend_index}"))?;
        let rasterizer = self
            .render_states
            .rasterizer_states
            .get(depth_bias_index)
            .and_then(|states| states.get(rasterizer_index))
            .with_context(|| {
                format!(
                    "invalid rasterizer/depth bias state index {rasterizer_index}/{depth_bias_index}"
                )
            })?;
        let (depth_stencil_reverse, depth_stencil_forward) = self
            .render_states
            .depth_stencil_states
            .get(depth_stencil_index)
            .with_context(|| format!("invalid depth/stencil state index {depth_stencil_index}"))?;
        let depth_stencil = match key.depth_mode {
            DepthMode::Reverse => depth_stencil_reverse,
            DepthMode::Forward => depth_stencil_forward,
        };

        let has_depth = key.depth_format.is_some();
        let has_stencil = key.depth_format.is_some_and(format_has_stencil);

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .alpha_to_coverage_enable(blend_state.alpha_to_coverage_enable);

        let rasterizer_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(rasterizer.polygon_mode)
            .cull_mode(rasterizer.cull_mode)
            .front_face(rasterizer.front_face)
            .depth_clamp_enable(rasterizer.depth_clamp_enable)
            .depth_bias_enable(rasterizer.depth_bias_enable)
            .depth_bias_constant_factor(rasterizer.depth_bias_constant_factor)
            .depth_bias_clamp(rasterizer.depth_bias_clamp)
            .depth_bias_slope_factor(rasterizer.depth_bias_slope_factor)
            .line_width(1.0);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(depth_stencil.depth_test_enable && has_depth)
            .depth_write_enable(
                depth_stencil.depth_write_enable && depth_stencil.depth_test_enable && has_depth,
            )
            .depth_compare_op(depth_stencil.depth_compare_op)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(depth_stencil.stencil_test_enable && has_stencil)
            .front(depth_stencil.front)
            .back(depth_stencil.back)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0);

        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(vertex_binding_descs)
            .vertex_attribute_descriptions(vertex_attribute_descs);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(key.topology)
            .primitive_restart_enable(true);

        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        let attachments = &blend_state.render_targets[..key.color_formats.len()];

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(blend_state.logic_op_enable)
            .logic_op(blend_state.logic_op)
            .attachments(attachments);
        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
            vk::DynamicState::PRIMITIVE_TOPOLOGY,
            vk::DynamicState::STENCIL_REFERENCE,
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

        #[cfg(debug_assertions)]
        {
            let name_cstr =
                CString::new(format!("vs_{}_ps_{}", key.vertex_shader, key.pixel_shader)).unwrap();
            let info = vk::DebugUtilsObjectNameInfoEXT::default()
                .object_name(&name_cstr)
                .object_handle(pipeline);

            unsafe {
                self.debug_loader.set_debug_utils_object_name(&info)?;
            };
        }

        Ok(pipeline)
    }

    pub fn get(&self, key: &PipelineKey) -> Option<vk::Pipeline> {
        self.storage.get(key).copied()
    }

    #[profiling::function]
    pub fn get_or_create(&mut self, key: &PipelineKey) -> anyhow::Result<vk::Pipeline> {
        if let Some(pipeline) = self.storage.get(key) {
            return Ok(*pipeline);
        }

        let layout = self.get_or_create_layout(key.vertex_shader, key.pixel_shader)?;
        let vs_info = self.get_or_load_bytecode(key.vertex_shader, ShaderStage::Vertex)?;
        let ps_info = self.get_or_load_bytecode(key.pixel_shader, ShaderStage::Pixel)?;

        let pipeline = self.get_or_create_pipeline(key, &layout, &vs_info, &ps_info)?;

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
                let data = if let Some(data) = self.builtin_shaders.get(&hash) {
                    Cow::Borrowed(*data)
                } else {
                    let tag_entry = package_manager()
                        .get_entry(hash)
                        .context("entry not found")?;
                    Cow::Owned(
                        package_manager()
                            .read_tag(tag_entry.reference)
                            .context("failed to read shader bytecode")?,
                    )
                };

                let (spirv, slots) = dxil2spirv::translate(data.as_ref(), set_base)
                    .map_err(|e| anyhow::anyhow!(e))?;

                let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv);
                let module = unsafe { self.device.create_shader_module(&create_info, None)? };

                Ok(entry
                    .insert(ShaderInfo {
                        module_tag: hash,
                        module,
                        slots,
                    })
                    .clone())
            }
        }
    }
}

const fn format_has_stencil(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::D16_UNORM_S8_UINT
            | vk::Format::D24_UNORM_S8_UINT
            | vk::Format::D32_SFLOAT_S8_UINT
    )
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
