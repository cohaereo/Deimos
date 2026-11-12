use std::any::TypeId;

use crate::tfx::externs::{
    BaseExternSource, Extern, ExternAccessor,
    macros::{extern_container, local_extern_container},
};
use deimos_data::{hash::fnv1, tfx::ExternIndex};
use glam::Vec4;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::definitions::*;

extern_container! {
    pub struct ExternContainer {
        frame: Frame,
        view: View,
        deferred: Deferred,
        deferred_light: DeferredLight,
        deferred_uber_light: DeferredUberLight,
        deferred_shadow: DeferredShadow,
        atmosphere: Atmosphere,
        dynamic_model: RigidModel,
        editor_mesh_material: EditorMeshMaterial,
        editor_decal: EditorDecal,
        editor_terrain: EditorTerrain,
        editor_terrain_patch: EditorTerrainPatch,
        editor_terrain_debug: EditorTerrainDebug,
        simple_geometry: SimpleGeometry,
        cui_object: CuiObject,
        cui_video: CuiVideo,
        cui_standard: CuiStandard,
        cui_screenspace_boxes: CuiScreenspaceBoxes,
        cui_drawing_shader: CuiDrawingShader,
        texture_visualizer: TextureVisualizer,
        generic: Generic,
        particle: Particle,
        particle_debug: ParticleDebug,
        screen_area: ScreenArea,
        msaa: Msaa,
        hdao: Hdao,
        downsample_texture_generic: DownsampleTextureGeneric,
        downsample_depth: DownsampleDepth,
        ssao: Ssao,
        postprocess: Postprocess,
        transparent: Transparent,
        vignette: Vignette,
        global_lighting: GlobalLighting,
        shadow_mask: ShadowMask,
        object_effect: ObjectEffect,
        decal: Decal,
        decal_set_transform: DecalSetTransform,
        dynamic_decal: DynamicDecal,
        decorator_wind: DecoratorWind,
        volume_fog: VolumeFog,
        fxaa: Fxaa,
        smaa: Smaa,
        cmaa: Cmaa,
        letterbox: Letterbox,
        depth_of_field: DepthOfField,
        postprocess_initial_downsample: PostprocessInitialDownsample,
        displacement_motion_blur: DisplacementMotionBlur,
        debug_shader: DebugShader,
        minmax_depth: MinmaxDepth,
        sdsm_bias_and_scale: SdsmBiasAndScale,
        sdsm_bias_and_scale_textures: SdsmBiasAndScaleTextures,
        compute_shadow_map_data: ComputeShadowMapData,
        compute_local_light_shadow_map_data: ComputeLocalLightShadowMapData,
        bilateral_upsample: BilateralUpsample,
        health_overlay: HealthOverlay,
        light_probe_dominant_light: LightProbeDominantLight,
        light_probe_light_instance: LightProbeLightInstance,
        water: Water,
        lens_flare: LensFlare,
        screen_shader: ScreenShader,
        scaler: Scaler,
        gamma_control: GammaControl,
        speedtree_placements: SpeedtreePlacements,
        reticle: Reticle,
        distortion: Distortion,
        water_depth_prepass: WaterDepthPrepass,
        overhead_visibility_map_main: OverheadVisibilityMapMain,
        overhead_visibility_map_interior: OverheadVisibilityMapInterior,
        overhead_visibility_map_occlusion: OverheadVisibilityMapOcclusion,
        particle_compute: ParticleCompute,
        cubemap_filtering: CubemapFiltering,
        cubemap_diffuse_capture: CubemapDiffuseCapture,
        volumetrics_pass: VolumetricsPass,
        temporal_reprojection: TemporalReprojection,
        uber_depth: UberDepth,
        cubemaps: Cubemaps,
        shadow_blend_with_previous: ShadowBlendWithPrevious,
        debug_shading_output: DebugShadingOutput,
        water_displacement: WaterDisplacement,
        pattern_blending: PatternBlending,
        ui_hdr_transform: UiHdrTransform,
        player_centered_cascaded_grid: PlayerCenteredCascadedGrid,
        soft_deform: SoftDeform,
        raymarched_atmosphere_volume: RaymarchedAtmosphereVolume,
        gtao: Gtao,
        taa: Taa,
        first_person_shadows: FirstPersonShadows,
        skinning_compute: SkinningCompute,
        ssr: Ssr,
        stylized_drop_shadow: StylizedDropShadow,
        ssr_trace: SsrTrace,
        texture_camera_downsample: TextureCameraDownsample,
        occlusion_culling_reproject_depth: OcclusionCullingReprojectDepth,
        occlusion_culling_visibility_testing: OcclusionCullingVisibilityTesting,
        variable_rate_shading: VariableRateShading
    }
}

impl ExternContainer {
    pub fn get_global_channel_by_name(&self, name: &str) -> Vec4 {
        self.get_global_channel_by_id(fnv1(name))
    }

    pub fn get_global_channel_by_id(&self, id: u32) -> Vec4 {
        self.global_ids
            .iter()
            .position(|i| *i == id)
            .map_or(Vec4::ONE, |pos| self.globals[pos])
    }

    /// Sets the value of the given global channel by ID
    /// Returns `Some` with the previous value if the channel exists, `None` otherwise
    pub fn set_global_channel_by_id(&mut self, id: u32, v: Vec4) -> Option<Vec4> {
        if let Some(pos) = self.global_ids.iter().position(|i| *i == id) {
            Some(std::mem::replace(&mut self.globals[pos], v))
        } else {
            None
        }
    }

    /// Sets the value of the given global channel by name, hashing the name to get its ID
    /// Returns `Some` with the previous value if the channel exists, `None` otherwise
    pub fn set_global_channel_by_name(&mut self, name: &str, v: Vec4) -> Option<Vec4> {
        self.set_global_channel_by_id(fnv1(name), v)
    }

    // /// Resets every global channel to their default value, as defined by render globals
    // pub fn reset_global_channels(&mut self) {
    //     self.globals[..globals.channels.default_values.len()]
    //         .copy_from_slice(&globals.channels.default_values);
    // }
}

local_extern_container! {
    rigid_model: RigidModel,
    speedtree_placements: SpeedtreePlacements,
    simple_geometry: SimpleGeometry,
    deferred_light: DeferredLight,
    decal: Decal,
    deferred_shadow: DeferredShadow
}

const GLOBAL_CHANNEL_NAMES: &[&str] = &[
    "autoexposure_zero_bias",
    "autoexposure_bias",
    "autoexposure_max_approach_speed",
    "autoexposure_adjust_speed",
    "autoexposure_frame_delay",
    "autoexposure_min_stops",
    "autoexposure_max_stops",
    "sun_intensity",
    "sun_color",
    "sun_glow_color",
    "sun_glow_shape",
    "sun_glow_intensity",
    "sky_sun_glow_intensity",
    "fog_density",
    "fog_density_lookup_start",
    "fog_density_lookup_end",
    "fog_start_height",
    "fog_height_falloff",
    "fog_decay_color",
    "fog_decay_scale",
    "layered_fog_density",
    "layered_fog_falloff",
    "layered_fog_start_height",
    "skybox_sun_intensity",
    "skybox_sun_color",
    "skybox_up_ambient_color",
    "skybox_up_ambient_intensity",
    "skybox_down_ambient_color",
    "skybox_down_ambient_intensity",
    "sky_color_override",
    "sky_snapshot_rotation",
    "sky_snapshot_intensity",
    "cubemap_splash_space_in",
    "sfx_reprojection_mipmap_hdr",
    "ao_ambient_weight",
    "ao_sun_weight",
    "up_ambient_color",
    "up_ambient_intensity",
    "up_ambient_sharpness",
    "down_ambient_color",
    "down_ambient_intensity",
    "down_ambient_sharpness",
    "sun_transmission_color",
    "sun_shadow_intensity",
    "sun_transmission_intensity",
    "duplex_involved",
    "sun_light_direction",
    "sun_atmosphere_direction",
    "sun_ambient_direction",
    "sun_track_direction",
    "sun_or_moon",
    "dc_intensity",
    "dc_color",
    "fx_channel_01",
    "fx_channel_02",
    "fx_channel_03",
    "fx_channel_04",
    "fx_channel_05",
    "character_lighting_scale_ao",
    "character_lighting_scale_emissive",
    "screenspace_reflection_intensity",
    "global_ambient_intensity",
    "global_cubemap_intensity",
    "global_cubemap_diffuse_intensity",
    "global_cubemap_down_color",
    "sun_direct_intensity",
    "cubemap_relighting_sky_intensity",
    "cubemap_bounce_scale",
    "cubemap_sky_intensity",
    "sun_tunnel_hash5",
    "sun_tunnel_hash4",
    "sun_tunnel_hash7",
    "reticle_location",
    "switch_volumetric_glow_hash2",
    "switch_volumetric_glow_hash1",
    "switch_volumetric_glow_hash0",
    "fog_density_interior",
    "reticle_utilize_filtering",
];

pub fn get_global_channel_name(hash: u32) -> Option<&'static str> {
    GLOBAL_CHANNEL_NAMES
        .iter()
        .copied()
        .find(|&name| fnv1(name) == hash)
}
