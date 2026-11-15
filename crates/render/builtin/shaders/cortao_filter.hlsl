#define CORTAO_HLSL 1
#define CORTAO_PERSPECTIVE_PROJECTION 1
#define CORTAO_16_BIT 0
#define CORTAO_LARGE_FILTER 1
#define CORTAO_FILTER_SYMMETRY 1

#include "cortao/CORTAOFilterPass.h"

cbuffer cortao_filter_params : register(b0) {
  CORTAOFilterPassParameters params;
};

Texture2D<uint> packed_data : register(t0);
Texture2D<float> occlusion_input : register(t1);

SamplerState nearest_border_sampler : register(s0);

RWTexture2D<float> occlusion_output : register(u0);

uint4 cortao_gatherPackedData(float2 texCoords, int2 offset) {
  return packed_data.Gather(nearest_border_sampler, texCoords, offset);
}

float4 cortao_gatherSSAO(float2 texCoords, int2 offset) {
  return occlusion_input.Gather(nearest_border_sampler, texCoords, offset);
}

void cortao_writeFilterOutput(int2 pixelCoords, float ssao) {
  occlusion_output[pixelCoords] = ssao;
}

[numthreads(8, 8, 1)]
void main(uint2 threadCoords : SV_DispatchThreadID) {
  cortao_filterPass(threadCoords, params);
}
