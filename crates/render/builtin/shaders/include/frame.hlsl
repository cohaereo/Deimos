#pragma once

cbuffer scope_frame : register(b13) {
    float game_time;
    float render_time;
    float delta_game_time;
    float exposure_time;

    float exposure_scale;
    float exposure_illum_relative_glow;
    float exposure_scale_for_shading;
    float exposure_illum_relative;
}; // cbuffer scope_frame
