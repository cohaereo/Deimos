#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
#![allow(
    clippy::missing_safety_doc,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::semicolon_if_nothing_returned,
    clippy::use_self
)]

use std::ptr::null_mut;

// cohae: link_cplusplus needs to be referenced as external crate in order to link stdc++
#[allow(unused_extern_crates)]
extern crate link_cplusplus;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Maximum number of resources we track per resource class.
pub const MAX_SLOTS: usize = 32;

/// Descriptor type a shader expects for a given resource slot.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotType {
    #[default]
    Unknown = 0,
    UniformBuffer,
    SampledImage,
    StorageImage,
    UniformTexelBuffer,
    StorageTexelBuffer,
    StorageBuffer,
    Sampler,
}

/// Bitmasks indicating which resource slots are used, plus the descriptor type of every used slot.
#[derive(Default, Clone)]
pub struct SlotUsage {
    pub cbv: u32,
    pub srv: u32,
    pub sampler: u32,
    pub uav: u32,

    pub srv_types: [SlotType; MAX_SLOTS],
    pub uav_types: [SlotType; MAX_SLOTS],
}

impl SlotUsage {
    pub const fn is_empty(&self) -> bool {
        self.cbv == 0 && self.srv == 0 && self.sampler == 0 && self.uav == 0
    }

    pub fn iter_mask(mask: u32) -> impl Iterator<Item = u32> {
        (0..MAX_SLOTS as u32).filter(move |slot| mask & (1 << slot) != 0)
    }
}

pub const SET_CBV: u32 = 0;
pub const SET_SRV: u32 = 1;
pub const SET_SAMPLER: u32 = 2;
pub const SET_UAV: u32 = 3;

/// Number of descriptor sets reserved per shader stage
pub const SETS_PER_STAGE: u32 = 4;

struct RemapperContext {
    slots: SlotUsage,
    set_base: u32,
}

pub fn translate(dxil_data: &[u8], set_base: u32) -> Result<(Vec<u32>, SlotUsage), String> {
    unsafe {
        let mut blob: *mut dxil_spv_parsed_blob_s = null_mut();
        if dxil_spv_parse_dxil_blob(dxil_data.as_ptr().cast(), dxil_data.len(), &mut blob)
            != dxil_spv_result_DXIL_SPV_SUCCESS
        {
            return Err("Failed to parse DXIL blob".to_string());
        }

        let mut converter: *mut dxil_spv_converter_s = null_mut();
        if dxil_spv_create_converter(blob, &mut converter) != dxil_spv_result_DXIL_SPV_SUCCESS {
            return Err("Failed to create converter".to_string());
        }

        let mut ctx = RemapperContext {
            slots: SlotUsage::default(),
            set_base,
        };
        let ctx_ptr = (&raw mut ctx).cast();

        dxil_spv_converter_set_srv_remapper(converter, Some(srv_remapper), ctx_ptr);
        dxil_spv_converter_set_sampler_remapper(converter, Some(sampler_remapper), ctx_ptr);
        dxil_spv_converter_set_uav_remapper(converter, Some(uav_remapper), ctx_ptr);
        dxil_spv_converter_set_cbv_remapper(converter, Some(cbv_remapper), ctx_ptr);

        if dxil_spv_converter_run(converter) != dxil_spv_result_DXIL_SPV_SUCCESS {
            return Err("Failed to run converter".to_string());
        }

        let mut compiled: dxil_spv_compiled_spirv = std::mem::zeroed();
        if dxil_spv_converter_get_compiled_spirv(converter, &mut compiled)
            != dxil_spv_result_DXIL_SPV_SUCCESS
        {
            return Err("Failed to get compiled SPIR-V".to_string());
        }

        let result = std::slice::from_raw_parts(compiled.data.cast(), compiled.size / 4).to_vec();

        Ok((result, ctx.slots))
    }
}

const fn is_texture_kind(kind: dxil_spv_resource_kind) -> bool {
    matches!(
        kind,
        dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_1D
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_2D
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_2DMS
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_3D
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_CUBE
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_1D_ARRAY
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_2D_ARRAY
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_2D_MS_ARRAY
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TEXTURE_CUBE_ARRAY
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_FEEDBACK_TEXTURE_2D
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_FEEDBACK_TEXTURE_2D_ARRAY
    )
}

const fn is_buffer_kind(kind: dxil_spv_resource_kind) -> bool {
    matches!(
        kind,
        dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TYPED_BUFFER
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_RAW_BUFFER
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_STRUCTURED_BUFFER
            | dxil_spv_resource_kind_DXIL_SPV_RESOURCE_KIND_TBUFFER
    )
}

const fn srv_slot_type(
    kind: dxil_spv_resource_kind,
    descriptor_type: dxil_spv_vulkan_descriptor_type,
) -> SlotType {
    if is_texture_kind(kind) {
        SlotType::SampledImage
    } else if is_buffer_kind(kind) {
        if descriptor_type == dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_SSBO
            || descriptor_type
                == dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_SSBO_SCALAR_ALIGNED
        {
            SlotType::StorageBuffer
        } else {
            SlotType::UniformTexelBuffer
        }
    } else {
        SlotType::Unknown
    }
}

const fn uav_slot_type(
    kind: dxil_spv_resource_kind,
    descriptor_type: dxil_spv_vulkan_descriptor_type,
) -> SlotType {
    if is_texture_kind(kind) {
        SlotType::StorageImage
    } else if is_buffer_kind(kind) {
        if descriptor_type == dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_SSBO
            || descriptor_type
                == dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_SSBO_SCALAR_ALIGNED
        {
            SlotType::StorageBuffer
        } else {
            SlotType::StorageTexelBuffer
        }
    } else {
        SlotType::Unknown
    }
}

unsafe extern "C" fn cbv_remapper(
    userdata: *mut ::std::os::raw::c_void,
    d3d_binding: *const dxil_spv_d3d_binding,
    vk_binding: *mut dxil_spv_cbv_vulkan_binding,
) -> dxil_spv_bool {
    unsafe {
        let ctx = &mut *(userdata.cast::<RemapperContext>());
        let register_index = (*d3d_binding).register_index;

        let binding = &mut (*vk_binding).vulkan.uniform_binding;
        binding.set = ctx.set_base + SET_CBV;
        binding.binding = register_index;
        binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;
        binding.bindless.use_heap = 0;
        (*vk_binding).push_constant = 0;

        if register_index >= MAX_SLOTS as u32 {
            eprintln!("Register index {} out of bounds for CBV", register_index);
        } else {
            ctx.slots.cbv |= 1 << register_index;
        }
    }

    1
}

unsafe extern "C" fn srv_remapper(
    userdata: *mut ::std::os::raw::c_void,
    d3d_binding: *const dxil_spv_d3d_binding,
    vk_binding: *mut dxil_spv_srv_vulkan_binding,
) -> dxil_spv_bool {
    unsafe {
        let ctx = &mut *(userdata.cast::<RemapperContext>());
        let register_index = (*d3d_binding).register_index;
        let kind = (*d3d_binding).kind;

        let binding = &mut (*vk_binding).buffer_binding;
        binding.set = ctx.set_base + SET_SRV;
        binding.binding = register_index;
        binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;
        binding.bindless.use_heap = 0;

        (*vk_binding).offset_binding = std::mem::zeroed();
        (*vk_binding).offset_binding.set = ctx.set_base + SET_SRV;
        (*vk_binding).offset_binding.binding = register_index;
        (*vk_binding).offset_binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;

        if register_index >= MAX_SLOTS as u32 {
            eprintln!("Register index {} out of bounds for SRV", register_index);
        } else {
            ctx.slots.srv |= 1 << register_index;
            ctx.slots.srv_types[register_index as usize] =
                srv_slot_type(kind, (*vk_binding).buffer_binding.descriptor_type);
        }
    }

    1
}

unsafe extern "C" fn sampler_remapper(
    userdata: *mut ::std::os::raw::c_void,
    d3d_binding: *const dxil_spv_d3d_binding,
    vk_binding: *mut dxil_spv_vulkan_binding,
) -> dxil_spv_bool {
    unsafe {
        let ctx = &mut *(userdata.cast::<RemapperContext>());
        let register_index = (*d3d_binding).register_index;

        (*vk_binding).set = ctx.set_base + SET_SAMPLER;
        (*vk_binding).binding = register_index;
        (*vk_binding).descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;
        (*vk_binding).bindless.use_heap = 0;

        if register_index >= MAX_SLOTS as u32 {
            eprintln!(
                "Register index {} out of bounds for Sampler",
                register_index
            );
        } else {
            ctx.slots.sampler |= 1 << register_index;
        }
    }

    1
}

unsafe extern "C" fn uav_remapper(
    userdata: *mut ::std::os::raw::c_void,
    d3d_binding: *const dxil_spv_uav_d3d_binding,
    vk_binding: *mut dxil_spv_uav_vulkan_binding,
) -> dxil_spv_bool {
    unsafe {
        let ctx = &mut *(userdata.cast::<RemapperContext>());
        let register_index = (*d3d_binding).d3d_binding.register_index;
        let kind = (*d3d_binding).d3d_binding.kind;

        if (*d3d_binding).has_counter != 0 {
            eprintln!(
                "UAV {} has a counter buffer, which is not supported",
                register_index
            );
        }

        let binding = &mut (*vk_binding).buffer_binding;
        binding.set = ctx.set_base + SET_UAV;
        binding.binding = register_index;
        binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;
        binding.bindless.use_heap = 0;

        (*vk_binding).counter_binding = std::mem::zeroed();
        (*vk_binding).counter_binding.set = ctx.set_base + SET_UAV;
        (*vk_binding).counter_binding.binding = register_index;
        (*vk_binding).counter_binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;

        (*vk_binding).offset_binding = std::mem::zeroed();
        (*vk_binding).offset_binding.set = ctx.set_base + SET_UAV;
        (*vk_binding).offset_binding.binding = register_index;
        (*vk_binding).offset_binding.descriptor_type =
            dxil_spv_vulkan_descriptor_type_DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;

        if register_index >= MAX_SLOTS as u32 {
            eprintln!("Register index {} out of bounds for UAV", register_index);
        } else {
            ctx.slots.uav |= 1 << register_index;
            ctx.slots.uav_types[register_index as usize] =
                uav_slot_type(kind, (*vk_binding).buffer_binding.descriptor_type);
        }
    }

    1
}
