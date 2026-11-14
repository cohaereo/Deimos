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

static const int MAX_STEPS = 128;
static const int REFINE_STEPS = 6;

float InterleavedGradientNoise(float2 position) {
  float3 magic = float3(0.06711056f, 0.00583715f, 52.9829189f);
  return frac(magic.z * frac(dot(position, magic.xy)));
}

float4 ToClip(float3 view_position) {
  return mul(camera_to_projective, float4(view_position, 1.0f));
}

float2 ClipToPixel(float4 clip) {
  float2 ndc = clip.xy / clip.w;
  return float2(ndc.x * 0.5f + 0.5f, 0.5f - ndc.y * 0.5f) * resolution.xy;
}

float3 ReconstructViewPosition(float2 pixel_pos, float depth) {
  float4 position = mul(target_pixel_to_camera, float4(pixel_pos, depth, 1.0f));
  return position.xyz / position.w;
}

bool SampleSurface(float2 p, out float surface_dist, out float thickness) {
  int2 pixel = int2(clamp(floor(p), 0.0f, resolution.xy - 1.0f));
  float depth = deferred_depth.Load(int3(pixel, 0));
  if (depth <= 0.0f) {
    surface_dist = 0.0f;
    thickness = 0.0f;
    return false;
  }
  float3 surface_v = ReconstructViewPosition(float2(pixel) + 0.5f, depth);
  surface_dist = -surface_v.z;
  thickness = max(0.05f, surface_dist * params.y);
  return true;
}

bool DecodeNormalView(float2 p, out float3 normal_v) {
  int2 pixel = int2(clamp(floor(p), 0.0f, resolution.xy - 1.0f));
  float3 raw = gbuffer_normal.Load(int3(pixel, 0)).xyz * 2.0f - 1.0f;
  float len = length(raw);
  if (len < 0.01f) {
    normal_v = 0.0f;
    return false;
  }
  normal_v = normalize(mul((float3x3)world_to_camera, raw / len));
  return true;
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

  float3 normal_v = normalize(mul((float3x3)world_to_camera, raw_normal / normal_length));
  float3 origin_v = ReconstructViewPosition(input.uv * resolution.xy, scene_depth);
  float3 to_surface = normalize(origin_v);
  float3 view_v = -to_surface;
  float no_v = saturate(dot(normal_v, view_v));
  if (no_v <= 0.0f) {
    return base_color;
  }

  float3 reflection_v = reflect(to_surface, normal_v);

  float noise_a = InterleavedGradientNoise(input.pos.xy);
  float noise_b = InterleavedGradientNoise(input.pos.xy + float2(47.0f, 17.0f));
  float noise_c = InterleavedGradientNoise(input.pos.xy + float2(113.0f, 71.0f));

  {
    float3 helper = abs(reflection_v.y) < 0.99f ? float3(0, 1, 0) : float3(1, 0, 0);
    float3 t = normalize(cross(helper, reflection_v));
    float3 b = cross(reflection_v, t);
    float2 n2 = float2(noise_a, noise_c) * 2.0f - 1.0f;
    reflection_v = normalize(reflection_v + (t * n2.x + b * n2.y) * roughness * roughness * 0.35f);
    float below = dot(reflection_v, normal_v);
    if (below < 0.01f) {
      reflection_v = normalize(reflection_v + normal_v * (0.01f - below));
    }
  }

  if (reflection_v.z >= 0.0f) {
    return base_color;
  }

  float max_distance = params.x * lerp(0.4f, 1.0f, smoothness);
  int step_count = clamp((int)params2.x, 1, MAX_STEPS);

  float ray_bias = max(0.02f, length(origin_v) * 0.002f);
  float3 ray_origin = origin_v + normal_v * ray_bias;
  float3 ray_end = ray_origin + reflection_v * max_distance;

  const float w_eps = 1e-4f;
  float4 c0 = ToClip(ray_origin);
  if (c0.w <= w_eps) return base_color;

  float4 c1 = ToClip(ray_end);
  if (c1.w < w_eps) {
    float t = (c0.w - w_eps) / (c0.w - c1.w);
    ray_end = lerp(ray_origin, ray_end, t);
    c1 = ToClip(ray_end);
  }

  float k0 = 1.0f / c0.w;
  float k1 = 1.0f / c1.w;
  float3 Q0 = ray_origin * k0;
  float3 Q1 = ray_end * k1;
  float2 p0 = ClipToPixel(c0);
  float2 p1 = ClipToPixel(c1);

  float2 delta = p1 - p0;
  float alpha = 1.0f;
  if (p1.x > resolution.x)
    alpha = min(alpha, (resolution.x - p0.x) / delta.x);
  else if (p1.x < 0.0f)
    alpha = min(alpha, (0.0f - p0.x) / delta.x);

  if (p1.y > resolution.y)
    alpha = min(alpha, (resolution.y - p0.y) / delta.y);
  else if (p1.y < 0.0f)
    alpha = min(alpha, (0.0f - p0.y) / delta.y);

  alpha = saturate(alpha);
  p1 = p0 + delta * alpha;
  k1 = lerp(k0, k1, alpha);
  Q1 = lerp(Q0, Q1, alpha);
  delta = p1 - p0;

  float pixel_len = max(abs(delta.x), abs(delta.y));
  if (pixel_len < 1.0f) {
    return base_color;
  }

  float stride = max(1.0f, pixel_len / (float)step_count);
  float du = stride / pixel_len;

  bool hit = false;
  float hit_u = 0.0f;

  float u_prev = noise_b * du;
  float prev_d = -(lerp(Q0.z, Q1.z, u_prev) / lerp(k0, k1, u_prev));

  [loop]
  for (int i = 1; i <= MAX_STEPS; ++i) {
    if (u_prev >= 1.0f) {
      break;
    }

    float u = min((i + noise_b) * du, 1.0f);
    float2 p = lerp(p0, p1, u);
    float ray_d = -(lerp(Q0.z, Q1.z, u) / lerp(k0, k1, u));

    float surf_d, thick;
    if (SampleSurface(p, surf_d, thick)) {
      if (ray_d >= surf_d && prev_d <= surf_d + thick) {
        float lo = u_prev;
        float hi = u;

        [unroll]
        for (int j = 0; j < REFINE_STEPS; ++j) {
          float mid = 0.5f * (lo + hi);
          float mid_d = -(lerp(Q0.z, Q1.z, mid) / lerp(k0, k1, mid));
          float ms, mt;
          if (SampleSurface(lerp(p0, p1, mid), ms, mt) && mid_d >= ms) {
            hi = mid;
          } else {
            lo = mid;
          }
        }

        float hi_d = -(lerp(Q0.z, Q1.z, hi) / lerp(k0, k1, hi));
        float2 hi_p = lerp(p0, p1, hi);
        float hs, ht;
        if (SampleSurface(hi_p, hs, ht)) {
          float residual = hi_d - hs;
          if (residual >= -ht && residual <= ht) {
            float3 hit_n;
            bool back_face = DecodeNormalView(hi_p, hit_n) && dot(hit_n, reflection_v) > 0.0f;
            if (!back_face) {
              hit = true;
              hit_u = hi;
              break;
            }
          }
        }
      }
    }

    prev_d = ray_d;
    u_prev = u;
  }

  if (!hit) return base_color;

  float2 hit_pixel = lerp(p0, p1, hit_u);
  float2 hit_uv = hit_pixel / resolution.xy;
  float3 hit_view = lerp(Q0, Q1, hit_u) / lerp(k0, k1, hit_u);
  float hit_distance = length(hit_view - ray_origin);

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
