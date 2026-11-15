#include "include/screen_space_vs.hlsl"

Texture2D<float4> scene_color : register(t0);
Texture2D<float> occlusion : register(t1);

float4 mainPS(VSOutput input) : SV_Target {
  int2 pixel = int2(input.pos.xy);
  float ao = saturate(occlusion.Load(int3(pixel, 0)));

  float4 color = scene_color.Load(int3(pixel, 0));
  return float4(color.rgb * lerp(1.0, ao, 1.0), color.a);
}
