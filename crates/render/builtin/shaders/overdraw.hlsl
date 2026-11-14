#include "include/screen_space_vs.hlsl"

// Pixel Shader
Texture2D source : register(t0);
SamplerState samplerState : register(s0);

float4 mainPS(VSOutput input) : SV_TARGET {
  float t = source.Sample(samplerState, input.uv).r / 2.0f;

  float3 col0 = float3(0.0, 0.0, 0.5); // Dark blue
  float3 col1 = float3(0.0, 0.0, 1.0); // Blue
  float3 col2 = float3(0.0, 1.0, 0.0); // Green
  float3 col3 = float3(1.0, 1.0, 0.0); // Yellow
  float3 col4 = float3(1.0, 0.0, 0.0); // Red

  float3 color = col0;
  color = lerp(color, col1, smoothstep(0.00, 0.25, t));
  color = lerp(color, col2, smoothstep(0.25, 0.50, t));
  color = lerp(color, col3, smoothstep(0.50, 0.75, t));
  color = lerp(color, col4, smoothstep(0.75, 1.00, t));

  return float4(color, 1.0);
}
