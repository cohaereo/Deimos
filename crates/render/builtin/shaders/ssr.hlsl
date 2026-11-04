#include "include/screen_space_vs.hlsl"

cbuffer ssr_constants : register(b0) {
  float4x4 world_to_camera : packoffset(c0);
  float4x4 camera_to_projective : packoffset(c4);
  float4x4 target_pixel_to_camera : packoffset(c8);

  // x=max distance, y=thickness scale, z=smoothness threshold, w=intensity
  float4 params : packoffset(c12);
  // x=max steps
  float4 params2 : packoffset(c13);
  float4 resolution : packoffset(c14);
};

Texture2D<float4> scene_color : register(t0);
Texture2D<float4> gbuffer_normal : register(t1);
Texture2D<float> deferred_depth : register(t2);
Texture2D<float4> gbuffer_third : register(t3);

SamplerState linear_sampler : register(s0);

static const int MAX_STEPS = 64;

float InterleavedGradientNoise(float2 position) {
  float3 magic = float3(0.06711056f, 0.00583715f, 52.9829189f);
  return frac(magic.z * frac(dot(position, magic.xy)));
}

bool ProjectToScreen(float3 view_position, out float2 uv) {
  float4 clip = mul(camera_to_projective, float4(view_position, 1.0f));
  if (clip.w <= 0.0001f) {
    uv = 0.0f;
    return false;
  }

  float3 ndc = clip.xyz / clip.w;
  uv = float2(ndc.x * 0.5f + 0.5f, 0.5f - ndc.y * 0.5f);
  return uv.x >= 0.0f && uv.x <= 1.0f && uv.y >= 0.0f && uv.y <= 1.0f;
}

float LoadDepth(float2 uv) {
  int2 pixel = int2(clamp(uv * resolution.xy, 0.0f, resolution.xy - 1.0f));
  return deferred_depth.Load(int3(pixel, 0));
}

float3 ReconstructViewPosition(float2 uv, float depth) {
  float4 position = mul(target_pixel_to_camera, float4(uv * resolution.xy, depth, 1.0f));
  return position.xyz / position.w;
}

float4 mainPS(VSOutput input) : SV_Target {
  int2 pixel = int2(input.pos.xy);
  float4 base_color = scene_color.Load(int3(pixel, 0));

  float scene_depth = deferred_depth.Load(int3(pixel, 0));
  if (scene_depth <= 0.0f) {
    return base_color;
  }

  float3 raw_normal = gbuffer_normal.Load(int3(pixel, 0)).xyz * 2.0f - 1.0f;
  float normal_length = length(raw_normal);
  if (normal_length < 0.01f) {
    return base_color;
  }

  float smoothness = saturate(normal_length * 4.0f - 3.0f);
  if (smoothness < params.z) {
    return base_color;
  }

  float metallic = gbuffer_third.Load(int3(pixel, 0)).r;
  float roughness = 1.0f - smoothness;

  float3 normal_v = normalize(mul((float3x3)world_to_camera, normalize(raw_normal)));
  float3 origin_v = ReconstructViewPosition(input.uv, scene_depth);
  float3 to_surface = normalize(origin_v);
  float3 view_v = -to_surface;
  float no_v = saturate(dot(normal_v, view_v));
  if (no_v <= 0.0f) {
    return base_color;
  }

  float3 reflection_v = reflect(to_surface, normal_v);

  float noise = InterleavedGradientNoise(input.pos.xy);
  reflection_v = normalize(
      reflection_v +
      (noise - 0.5f) * roughness * roughness * 0.35f * float3(1.0f, 1.0f, 0.5f));

  if (reflection_v.z >= 0.0f) {
    return base_color;
  }

  float max_distance = params.x * lerp(0.4f, 1.0f, smoothness);
  int step_count = clamp((int)params2.x, 1, MAX_STEPS);
  float step_size = max_distance / (float)step_count;
  float thickness_scale = params.y;
  float ray_bias = max(0.02f, length(origin_v) * 0.002f);
  float3 ray_origin = origin_v + normal_v * ray_bias;

  bool hit = false;
  float2 hit_uv = 0.0f;
  float hit_distance = 0.0f;
  float previous_delta = -1.0f;

  [loop]
  for (int i = 1; i <= MAX_STEPS; ++i) {
    if (i > step_count) {
      break;
    }

    float distance = i * step_size;
    float3 position_v = ray_origin + reflection_v * distance;

    float2 uv;
    if (!ProjectToScreen(position_v, uv)) {
      break;
    }

    float surface_depth = LoadDepth(uv);
    if (surface_depth <= 0.0f) {
      previous_delta = -1.0f;
      continue;
    }

    float3 surface_v = ReconstructViewPosition(uv, surface_depth);
    float delta = -position_v.z - -surface_v.z;
    float thickness = max(0.05f, -surface_v.z * thickness_scale);

    if (delta >= 0.0f && delta <= thickness) {
      hit = true;
      hit_uv = uv;
      hit_distance = distance;
      break;
    }

    if (delta > thickness && previous_delta < 0.0f) {
      float low = distance - step_size;
      float high = distance;

      [unroll]
      for (int j = 0; j < 4; ++j) {
        float mid = (low + high) * 0.5f;
        float3 mid_v = ray_origin + reflection_v * mid;
        float2 mid_uv;

        if (!ProjectToScreen(mid_v, mid_uv)) {
          low = mid;
          continue;
        }

        float mid_depth = LoadDepth(mid_uv);
        if (mid_depth <= 0.0f) {
          low = mid;
          continue;
        }

        float3 mid_surface = ReconstructViewPosition(mid_uv, mid_depth);
        if (-mid_v.z - -mid_surface.z >= 0.0f) {
          high = mid;
        } else {
          low = mid;
        }
      }

      float3 hit_v = ray_origin + reflection_v * high;
      float2 refined_uv;
      if (ProjectToScreen(hit_v, refined_uv)) {
        float refined_depth = LoadDepth(refined_uv);
        if (refined_depth > 0.0f) {
          float3 refined_surface = ReconstructViewPosition(refined_uv, refined_depth);
          float refined_delta = -hit_v.z - -refined_surface.z;
          float refined_thickness =
              max(0.05f, -refined_surface.z * thickness_scale);

          if (refined_delta >= 0.0f && refined_delta <= refined_thickness) {
            hit = true;
            hit_uv = refined_uv;
            hit_distance = high;
          }
        }
      }

      if (hit) {
        break;
      }
    }

    previous_delta = delta;
  }

  if (!hit) {
    return base_color;
  }

  float3 reflection_color = scene_color.SampleLevel(linear_sampler, hit_uv, 0).rgb;

  float2 edge = saturate(hit_uv * 20.0f) * saturate((1.0f - hit_uv) * 20.0f);
  float edge_fade = edge.x * edge.y;
  float distance_fade = saturate(1.0f - hit_distance / max(max_distance, 0.0001f));
  distance_fade *= distance_fade;

  float3 f0 = lerp(float3(0.04f, 0.04f, 0.04f), float3(1.0f, 1.0f, 1.0f), metallic);
  float3 fresnel = f0 + (1.0f - f0) * pow(1.0f - no_v, 5.0f);
  float fresnel_scalar = dot(fresnel, float3(0.333333f, 0.333333f, 0.333333f));
  float specular_weight = lerp(fresnel_scalar, 1.0f, metallic);
  float weight = specular_weight * smoothness * smoothness;
  weight *= edge_fade * distance_fade * params.w;

  float3 result = lerp(base_color.rgb, reflection_color, saturate(weight));
  return float4(result, 1.0f);
}
