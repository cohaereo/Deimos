#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
#![allow(
    clippy::missing_safety_doc,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::semicolon_if_nothing_returned,
    clippy::use_self
)]

use std::ptr::null_mut;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Bitmasks indicating which resource slots are used.
#[derive(Default, Clone)]
pub struct SlotUsage {
    pub cbv: u32,
    pub srv: u32,
    pub sampler: u32,
    pub uav: u32,
}

pub const SET_CBV: u32 = 0;
pub const SET_SRV: u32 = 1;
pub const SET_SAMPLER: u32 = 2;
pub const SET_UAV: u32 = 3;

// cohae: link_cplusplus needs to be referenced as external crate in order to link stdc++
#[allow(unused_extern_crates)]
extern crate link_cplusplus;

pub fn translate(dxil_data: &[u8]) -> Result<(Vec<u32>, SlotUsage), String> {
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

        let mut slots = SlotUsage::default();

        dxil_spv_converter_set_srv_remapper(converter, Some(srv_remapper), (&raw mut slots).cast());
        dxil_spv_converter_set_sampler_remapper(
            converter,
            Some(sampler_remapper),
            (&raw mut slots).cast(),
        );
        dxil_spv_converter_set_uav_remapper(converter, Some(uav_remapper), (&raw mut slots).cast());
        dxil_spv_converter_set_cbv_remapper(converter, Some(cbv_remapper), (&raw mut slots).cast());

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

        Ok((result, slots))
    }
}

unsafe extern "C" fn cbv_remapper(
    userdata: *mut ::std::os::raw::c_void,
    d3d_binding: *const dxil_spv_d3d_binding,
    vk_binding: *mut dxil_spv_cbv_vulkan_binding,
) -> dxil_spv_bool {
    unsafe {
        let slots = &mut *(userdata.cast::<SlotUsage>());
        let register_index = (*d3d_binding).register_index;
        (*vk_binding).vulkan.uniform_binding.set = SET_CBV; // (*d3d_binding).register_space;
        (*vk_binding).vulkan.uniform_binding.binding = register_index;

        if register_index >= 32 {
            eprintln!("Register index {} out of bounds for CBV", register_index);
        } else {
            slots.cbv |= 1 << register_index;
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
        let slots = &mut *(userdata.cast::<SlotUsage>());
        let register_index = (*d3d_binding).register_index;
        (*vk_binding).buffer_binding.set = SET_SRV; // (*d3d_binding).register_space;
        (*vk_binding).buffer_binding.binding = register_index;

        if register_index >= 32 {
            eprintln!("Register index {} out of bounds for SRV", register_index);
        } else {
            slots.srv |= 1 << register_index;
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
        let slots = &mut *(userdata.cast::<SlotUsage>());
        let register_index = (*d3d_binding).register_index;
        (*vk_binding).set = SET_SAMPLER; // (*d3d_binding).register_space;
        (*vk_binding).binding = register_index;

        if register_index >= 32 {
            eprintln!(
                "Register index {} out of bounds for Sampler",
                register_index
            );
        } else {
            slots.sampler |= 1 << register_index;
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
        let slots = &mut *(userdata.cast::<SlotUsage>());
        let register_index = (*d3d_binding).d3d_binding.register_index;
        (*vk_binding).buffer_binding.set = SET_UAV; // (*d3d_binding).d3d_binding.register_space;
        (*vk_binding).buffer_binding.binding = register_index;

        if register_index >= 32 {
            eprintln!("Register index {} out of bounds for UAV", register_index);
        } else {
            slots.uav |= 1 << register_index;
        }
    }

    1
}
