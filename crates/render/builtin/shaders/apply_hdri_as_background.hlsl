#include "include/screen_space_vs.hlsl"
#include "include/view.hlsl"
#include "include/frame.hlsl"

// Pixel Shader
Texture2D<float> deferred_depth : register(t0);
Texture2D<float4> hdri : register(t1);
SamplerState samplerState : register(s0);

static const float PI = 3.14159265359f;

float2 DirToEquirect(float3 dir) {
  float phi = atan2(dir.y, dir.x);
  float theta = asin(clamp(dir.z, -1.0f, 1.0f));
  return float2(phi / (2.0f * PI) + 0.5f, 0.5f - theta / PI);
}

float3 SampleHDRI(float3 dir, float mip) {
  return hdri.SampleLevel(samplerState, DirToEquirect(dir), mip).rgb * 2.0;
}

float4 mainPS(VSOutput input) : SV_TARGET {
  float depth = deferred_depth.Load(int3(int2(input.pos.xy), 0));

  if (depth != 0)
    discard;

  float4 worldPos = mul(target_pixel_to_camera, float4(input.pos.xy, depth, 1));
  worldPos /= worldPos.w;
  worldPos = mul(camera_to_world, worldPos);
  worldPos /= worldPos.w;

  float3 viewDir = normalize(worldPos.xyz - camera_position);
  float3 skyColor = SampleHDRI(viewDir, 0.0f);

  // Output directly to the light buffers
  return float4(skyColor * exposure_scale, 1.0f);
}
