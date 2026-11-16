cbuffer cortao_temporal_params : register(b0) {
  uint packed_resolution;
  float temporal_alpha;
  float depth_reject_threshold;
  float _padding;
  float4 unproject_params;
  float4x4 cur_inv_world_to_camera;
  float4x4 prev_world_to_camera;
  float4 prev_projection_params;
};

Texture2D<float> filtered_input : register(t0);
Texture2D<uint> packed_data : register(t1);
Texture2D<uint> prev_packed : register(t2);
Texture2D<float> history : register(t3);

SamplerState nearest_clamp_sampler : register(s0);
SamplerState nearest_border_sampler : register(s1);
SamplerState linear_clamp_sampler : register(s2);

RWTexture2D<float> history_write : register(u0);

float unpackLinearDepth(uint packed) {
  return asfloat(packed);
}

[numthreads(8, 8, 1)]
void main(uint2 pixelCoords : SV_DispatchThreadID) {
  uint2 resolution = uint2(packed_resolution & 0xFFFFu, packed_resolution >> 16u);
  if (pixelCoords.x >= resolution.x || pixelCoords.y >= resolution.y) {
    return;
  }

  float current = filtered_input.Load(int3(int2(pixelCoords), 0));
  float blend = temporal_alpha;

  float2 texCoords = (pixelCoords + 0.5) / resolution;
  float2 prevTexCoords = texCoords;
  float historyValue = 0.0;
  float linearDepth = unpackLinearDepth(packed_data.SampleLevel(nearest_clamp_sampler, texCoords, 0.0));

  if (linearDepth > 0.0) {
    float2 direction = texCoords * unproject_params.xy + unproject_params.zw;
    float3 viewPosition = float3(direction * linearDepth, -linearDepth);
    float3 worldPosition = mul(cur_inv_world_to_camera, float4(viewPosition, 1.0)).xyz;

    float3 prevView = mul(prev_world_to_camera, float4(worldPosition, 1.0)).xyz;
    float3x3 prevProjection = float3x3(
      prev_projection_params.x, 0.0, 0.0,
      0.0, prev_projection_params.y, 0.0,
      prev_projection_params.zw, -1.0
    );
    float3 projected = mul(prevView, prevProjection);
    prevTexCoords = projected.xy / projected.z;

    float prevDepth = unpackLinearDepth(prev_packed.SampleLevel(nearest_border_sampler, prevTexCoords, 0.0));
    float expectedPrevDepth = -prevView.z;

    if (prevDepth <= 0.0 || abs(prevDepth - expectedPrevDepth) > depth_reject_threshold * expectedPrevDepth) {
      blend = 1.0;
    } else {
      historyValue = history.SampleLevel(linear_clamp_sampler, prevTexCoords, 0.0);
    }
  } else {
    blend = 1.0;
  }

  float neighborhood_min = current;
  float neighborhood_max = current;
  [unroll]
  for (int y = -1; y <= 1; y++) {
    [unroll]
    for (int x = -1; x <= 1; x++) {
      float sample_value = filtered_input.Load(int3(int2(pixelCoords) + int2(x, y), 0));
      neighborhood_min = min(neighborhood_min, sample_value);
      neighborhood_max = max(neighborhood_max, sample_value);
    }
  }

  float prev = clamp(historyValue, neighborhood_min, neighborhood_max);
  history_write[pixelCoords] = lerp(prev, current, blend);
}
