#define CORTAO_HLSL 1
#define CORTAO_PERSPECTIVE_PROJECTION 1
#define CORTAO_16_BIT 0
#define CORTAO_SAMPLE_COUNT 8
#define CORTAO_RAY_COUNT 16
#define CORTAO_OCCLUDER_LOOKUP_TABLE 1
#define CORTAO_BENT_NORMAL_LOOKUP_TABLE 0

#include "cortao/CORTAOMainPass.h"

cbuffer cortao_main_params : register(b0) {
  CORTAOMainPassParameters params;
  float4x4 world_to_camera;
};

Texture2D<uint> packed_data : register(t0);
Texture2D<float4> gbuffer_normal : register(t1);
Texture3D<uint> occluder_lookup_table : register(t2);

SamplerState nearest_border_sampler : register(s0);
SamplerState nearest_clamp_sampler : register(s1);

RWTexture2D<float> occlusion_output : register(u0);

uint cortao_samplePackedData(float2 texCoords, float mipmap) {
  return packed_data.SampleLevel(nearest_border_sampler, texCoords, mipmap);
}

float3 cortao_loadNormal(int2 pixelCoords) {
  float3 normal = gbuffer_normal.Load(int3(pixelCoords, 0)).xyz * 2.0 - 1.0;
  return normalize(mul((float3x3)world_to_camera, normal));
}

uint cortao_sampleOccluderLookupTable(float3 texCoords) {
  return occluder_lookup_table.SampleLevel(nearest_clamp_sampler, texCoords, 0.0);
}

void cortao_writeResult(int2 pixelCoords, float occlusion, float3 bentNormal, float3x3 tangentSpaceTransform, uint rayBits) {
  occlusion_output[pixelCoords] = occlusion;
}

uint2 deinterleave(uint index) {
  uint2 p = uint2(index, index) >> uint2(0, 1);

  p &= 0x5555u;
  p = (p ^ (p >> 1u)) & 0x3333u;
  p = (p ^ (p >> 2u)) & 0x0f0fu;
  p = (p ^ (p >> 4u)) & 0x00ffu;

  return p;
}

#define WORKGROUP_SIZE 16

[numthreads(WORKGROUP_SIZE, WORKGROUP_SIZE, 1)]
void main(uint2 workgroupID : SV_GroupID, uint localIndex : SV_GroupIndex) {
  uint2 pixelCoords = workgroupID * uint2(WORKGROUP_SIZE, WORKGROUP_SIZE) + deinterleave(localIndex);
  cortao_mainPass(pixelCoords, params);
}
