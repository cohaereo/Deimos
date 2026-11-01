use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use deimos_data::tfx::{
    features::cubemap::CubemapShape,
    render_globals::{SRenderGlobals, SRenderGlobalsData, SRenderGlobalsGlobalChannels},
};
use lazy_static::lazy_static;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    asset::{AssetManager, texture::Texture},
    gpu::Gpu,
    tfx::{scope::Scope, technique::Technique},
};

lazy_static! {
    // static ref GLOBAL_SCOPES: Vec<ScopeSamplers> = {
    //     // TODO(cohae): Feels a bit weird to load globals again here, but right now it's the most convenient way to access them for building root signatures
    //     let data: SRenderGlobals = package_manager()
    //         .read_named_tag_struct("render_globals")
    //         .expect("no render_globals tag");
    //     let globs = &data.unk8.first().expect("No render globals found").unk8.0;

    //     let mut scopes = Vec::new();
    //     for scope_tag in &globs.scopes {
    //         let scope = ScopeSamplers::load(scope_tag.scope).unwrap_or_else(|_| {
    //             panic!(
    //                 "failed to load scope {} ({})",
    //                 scope_tag.name.0, scope_tag.scope
    //             )
    //         });
    //         scopes.push(scope);
    //     }

    //     scopes
    // };

    pub static ref GLOBAL_CHANNELS: SRenderGlobalsGlobalChannels = {
        // TODO(cohae): Feels a bit weird to load globals again here, but right now it's the most convenient way to access them for building root signatures
        let data: SRenderGlobals = package_manager()
            .read_named_tag_struct("render_globals")
            .expect("no render_globals tag");
        let globs = &data.unk8.first().expect("No render globals found").unk8.0;

        globs.global_channels.0.clone()
    };
}

// pub fn try_get_scope_samplers(index: u32) -> Option<&'static ScopeSamplers> {
//     GLOBAL_SCOPES.get(index as usize)
// }

// pub fn get_scope_samplers(index: u32) -> &'static ScopeSamplers {
//     try_get_scope_samplers(index).unwrap_or_else(|| panic!("Scope {index} does not exist"))
// }

pub struct RenderGlobals {
    pub scopes: GlobalScopes,
    pub pipelines: GlobalPipelines,
    pub textures: GlobalTextures,
    pub channels: SRenderGlobalsGlobalChannels,
    // pub unk34: SUnk8080822d,
}

impl RenderGlobals {
    pub fn load(asset_manager: &Arc<AssetManager>, gpu: &Arc<Gpu>) -> anyhow::Result<Self> {
        let data: SRenderGlobals = package_manager().read_named_tag_struct("render_globals")?;
        let globs = &data.unk8.first().context("No render globals found")?.unk8.0;

        Ok(Self {
            scopes: GlobalScopes::load(gpu, asset_manager, globs),
            pipelines: GlobalPipelines::load(asset_manager, gpu, globs),
            textures: GlobalTextures::load(gpu, globs)?,
            channels: globs.global_channels.0.clone(),
            // unk34: globs.unk34.0.clone(),
        })
    }
}
pub struct GlobalTextures {
    pub specular_tint_lookup: Texture,
    pub specular_lobe_lookup: Texture,
    pub specular_lobe_3d_lookup: Texture,
    pub iridescence_lookup: Texture,
    pub water_displacement_unk00: Texture,
    pub water_displacement_unk08: Texture,
}

impl GlobalTextures {
    pub fn load(gpu: &Arc<Gpu>, data: &SRenderGlobalsData) -> anyhow::Result<Self> {
        Ok(Self {
            specular_tint_lookup: Texture::load_tag(gpu, data.unk30.specular_tint_lookup_texture)?,
            specular_lobe_lookup: Texture::load_tag(gpu, data.unk30.specular_lobe_lookup_texture)?,
            specular_lobe_3d_lookup: Texture::load_tag(
                gpu,
                data.unk30.specular_lobe_3d_lookup_texture,
            )?,
            iridescence_lookup: Texture::load_tag(gpu, data.unk30.iridescence_lookup_texture)?,
            water_displacement_unk00: Texture::load_tag(gpu, data.unk3c.water_displacement_unk00)?,
            water_displacement_unk08: Texture::load_tag(gpu, data.unk3c.water_displacement_unk08)?,
        })
    }
}

macro_rules! tfx_global_scopes {
    ($($name:ident),*) => {
        pub struct GlobalScopes {
            $(
                pub $name: Box<Scope>,
            )*
        }

        impl GlobalScopes {
            pub fn load(gpu: &Gpu, asset_manager: &Arc<AssetManager>, globals: &SRenderGlobalsData) -> Self {
                let scopes: HashMap<String, TagHash> = globals.scopes.iter().map(|p| (p.name.to_string(), p.scope)).collect();

                Self {
                    $(
                        $name: Box::new(Scope::load(
                            gpu,
                            asset_manager,
                            *scopes.get(stringify!($name))
                                .expect(&format!("Scope {} does not exist", stringify!($name))),
                        )
                        .expect(&format!("Failed to load scope {}", stringify!($name)))),
                    )*
                }
            }
        }
    };
}

tfx_global_scopes! {
    frame, view, rigid_model, editor_mesh, editor_terrain,
    cui_view, cui_object, skinning, speedtree, chunk_model,
    decal, instances, speedtree_lod_drawcall_data, transparent,
    transparent_advanced, sdsm_bias_and_scale_textures, terrain,
    postprocess, cui_bitmap, cui_standard, ui_font, cui_hud,
    particle_transforms, particle_location_metadata, cubemap_volume,
    gear_plated_textures, generic_array
}

macro_rules! tfx_global_pipelines {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        pub struct GlobalPipelines {
            $(
                pub $name: Box<Technique>,
            )*
        }


        impl GlobalPipelines {
            pub fn load(asset_manager: &Arc<AssetManager>, gpu: &Arc<Gpu>, globals: &SRenderGlobalsData) -> Self {
                let techniques: HashMap<String, TagHash> = globals.pipelines.iter().map(|p| (p.name.to_string(), p.technique)).collect();

                Self {
                    $(
                        $name: Technique::load(
                            asset_manager,
                            gpu,
                            *techniques.get(stringify!($name))
                                .expect(&format!("Technique {} does not exist", stringify!($name)))
                        )
                        .unwrap_or_else(|e| panic!("Failed to read global pipeline technique {}: {e:?}", stringify!($name))),
                    )*
                }
            }
        }
    };
}

tfx_global_pipelines! {
    // Shading
    clear_color_2_mrt,
    deferred_shading,
    deferred_shading_no_atm,
    global_lighting,
    global_lighting_and_shading,
    global_lighting_and_shading_gel,
    final_combine_no_film_curve,
    final_combine,

    // Post
    hdao,
    apply_ssao_to_light_buffers,
    ssao_bilateral_filter,
    // ssao_compute_ao_3D_ps,
    fxaa,
    fxaa_noise,
    autoexposure_sample_columns,

    // Utility
    copy_texture_bilinear,
    copy_texture_bilinear_tiled,

    // Cubemap variants
    cubemap_apply_cube_alpha_off_probes_off_relighting_off, cubemap_apply_cube_alpha_off_probes_off_relighting_on,
    cubemap_apply_cube_alpha_off_probes_on_relighting_off, cubemap_apply_cube_alpha_off_probes_on_relighting_on,
    cubemap_apply_cube_alpha_on_probes_off_relighting_off, cubemap_apply_cube_alpha_on_probes_off_relighting_on,
    cubemap_apply_cube_alpha_on_probes_on_relighting_off, cubemap_apply_cube_alpha_on_probes_on_relighting_on,
    cubemap_apply_cube_sphere_alpha_off_probes_off_relighting_off, cubemap_apply_cube_sphere_alpha_off_probes_off_relighting_on,
    cubemap_apply_cube_sphere_alpha_off_probes_on_relighting_off, cubemap_apply_cube_sphere_alpha_off_probes_on_relighting_on,
    cubemap_apply_cube_sphere_alpha_on_probes_off_relighting_off, cubemap_apply_cube_sphere_alpha_on_probes_off_relighting_on,
    cubemap_apply_cube_sphere_alpha_on_probes_on_relighting_off, cubemap_apply_cube_sphere_alpha_on_probes_on_relighting_on,
    cubemap_apply_sphere_alpha_off_probes_off_relighting_off, cubemap_apply_sphere_alpha_off_probes_off_relighting_on,
    cubemap_apply_sphere_alpha_off_probes_on_relighting_off, cubemap_apply_sphere_alpha_off_probes_on_relighting_on,
    cubemap_apply_sphere_alpha_on_probes_off_relighting_off, cubemap_apply_sphere_alpha_on_probes_off_relighting_on,
    cubemap_apply_sphere_alpha_on_probes_on_relighting_off, cubemap_apply_sphere_alpha_on_probes_on_relighting_on,
    cubemap_apply_parall_cube_alpha_off_probes_off_relighting_off, cubemap_apply_parall_cube_alpha_off_probes_off_relighting_on,
    cubemap_apply_parall_cube_alpha_off_probes_on_relighting_off, cubemap_apply_parall_cube_alpha_off_probes_on_relighting_on,
    cubemap_apply_parall_cube_alpha_on_probes_off_relighting_off, cubemap_apply_parall_cube_alpha_on_probes_off_relighting_on,
    cubemap_apply_parall_cube_alpha_on_probes_on_relighting_off, cubemap_apply_parall_cube_alpha_on_probes_on_relighting_on,
    cubemap_apply_parall_cube_sphere_alpha_off_probes_off_relighting_off, cubemap_apply_parall_cube_sphere_alpha_off_probes_off_relighting_on,
    cubemap_apply_parall_cube_sphere_alpha_off_probes_on_relighting_off, cubemap_apply_parall_cube_sphere_alpha_off_probes_on_relighting_on,
    cubemap_apply_parall_cube_sphere_alpha_on_probes_off_relighting_off, cubemap_apply_parall_cube_sphere_alpha_on_probes_off_relighting_on,
    cubemap_apply_parall_cube_sphere_alpha_on_probes_on_relighting_off, cubemap_apply_parall_cube_sphere_alpha_on_probes_on_relighting_on,
    cubemap_apply_parall_sphere_alpha_off_probes_off_relighting_off, cubemap_apply_parall_sphere_alpha_off_probes_off_relighting_on,
    cubemap_apply_parall_sphere_alpha_off_probes_on_relighting_off, cubemap_apply_parall_sphere_alpha_off_probes_on_relighting_on,
    cubemap_apply_parall_sphere_alpha_on_probes_off_relighting_off, cubemap_apply_parall_sphere_alpha_on_probes_off_relighting_on,
    cubemap_apply_parall_sphere_alpha_on_probes_on_relighting_off, cubemap_apply_parall_sphere_alpha_on_probes_on_relighting_on,

    cubemap_apply_sky_copy_ao,
    sky_generate_sky_mask,
    sky_lookup_generate_near, sky_lookup_generate_far,
    sky,

    debug_cubemap_diffuse_probes,
    debug_source_color,
    debug_specular_smoothness,
    debug_metalness,
    debug_texture_ao,
    debug_ambient_occlusion,
    debug_emissive,
    debug_emissive_intensity,
    debug_transmission,
    debug_colored_overcoat_id,
    debug_depth_edges,
    debug_world_normal,
    debug_diffuse_light,
    debug_specular_light,
    overdraw_visualizer,

    // LUT3D variants
    screen_area_global_lut3d_distort,
    screen_area_global_lut3d_distort_hdr,
    screen_area_global_lut3d_distort_noise,
    screen_area_global_lut3d_distort_noise_hdr,
    screen_area_global_lut3d,
    screen_area_global_lut3d_hdr,
    screen_area_global_lut3d_noise,
    screen_area_global_lut3d_noise_hdr,
    screen_area_global_lut3d_no_tonemap,

    downsample_depth_buffer, uber_depth_default, downsample_max_min_avg_no_swizzle,

    bloom_initial_downsample_block_2x2, downsample_block_2x2_with_nan_kill, downsample_block_2x2,
    downsample_gaussian_1x8, downsample_gaussian_8x1,
    downsample_gaussian_1x16, downsample_gaussian_16x1,

    gaussian_10_horz, gaussian_10_vert,
    weighted_6_horz, weighted_6_vert,
    combined_bloom_line_blur,
    radial_blur_8,

    weighted_add,

    volumetrics_upres_1,

    water_sky_color_generate,
    water_reflection_healing,
    water_reflection_resolve,
    water_reflection_uv_healing
}

impl GlobalPipelines {
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn get_specialized_cubemap_pipeline(
        &self,
        shape: CubemapShape,
        alpha: bool,
        probes: bool,
        relighting: bool,
        parallax: bool,
    ) -> &Technique {
        let pipeline_list = [
            // No Parallax
            [
                // Cube
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_cube_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_cube_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_cube_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_cube_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_cube_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_cube_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_cube_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_cube_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
                // Sphere
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_sphere_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_sphere_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_sphere_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_sphere_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_sphere_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_sphere_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_sphere_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_sphere_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
                // CubeSphere
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_cube_sphere_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_cube_sphere_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_cube_sphere_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_cube_sphere_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_cube_sphere_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_cube_sphere_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_cube_sphere_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_cube_sphere_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
            ],
            // Parallax
            [
                // ParallCube
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_cube_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_parall_cube_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_cube_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_parall_cube_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_cube_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_parall_cube_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_cube_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_parall_cube_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
                // ParallSphere
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_sphere_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_parall_sphere_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_sphere_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_parall_sphere_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_sphere_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_parall_sphere_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_sphere_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_parall_sphere_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
                // ParallCubeSphere
                [
                    // Alpha Off
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_cube_sphere_alpha_off_probes_off_relighting_off,
                            &self.cubemap_apply_parall_cube_sphere_alpha_off_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_cube_sphere_alpha_off_probes_on_relighting_off,
                            &self.cubemap_apply_parall_cube_sphere_alpha_off_probes_on_relighting_on,
                        ],
                    ],
                    // Alpha On
                    [
                        // Probes Off
                        [
                            &self.cubemap_apply_parall_cube_sphere_alpha_on_probes_off_relighting_off,
                            &self.cubemap_apply_parall_cube_sphere_alpha_on_probes_off_relighting_on,
                        ],
                        // Probes On
                        [
                            &self.cubemap_apply_parall_cube_sphere_alpha_on_probes_on_relighting_off,
                            &self.cubemap_apply_parall_cube_sphere_alpha_on_probes_on_relighting_on,
                        ],
                    ],
                ],
            ],
        ];

        pipeline_list[parallax as usize][shape as usize][alpha as usize][probes as usize]
            [relighting as usize]
    }

    pub fn get_specialized_lut3d_pipeline(
        &self,
        distort: bool,
        hdr: bool,
        noise: bool,
    ) -> &Technique {
        let pipeline_list = [
            // No Distort
            [
                // No Noise
                [
                    &self.screen_area_global_lut3d,
                    &self.screen_area_global_lut3d_hdr,
                ],
                // Noise
                [
                    &self.screen_area_global_lut3d_noise,
                    &self.screen_area_global_lut3d_noise_hdr,
                ],
            ],
            // Distort
            [
                // No Noise
                [
                    &self.screen_area_global_lut3d_distort,
                    &self.screen_area_global_lut3d_distort_hdr,
                ],
                // Noise
                [
                    &self.screen_area_global_lut3d_distort_noise,
                    &self.screen_area_global_lut3d_distort_noise_hdr,
                ],
            ],
        ];

        pipeline_list[distort as usize][noise as usize][hdr as usize]
    }
}
