#define CORTAO_HLSL 1
#define CORTAO_PERSPECTIVE_PROJECTION 1
#define CORTAO_PACKED_NORMAL_BITS 10

#include "cortao/CORTAOPrepass.h"

cbuffer cortao_prepass_params : register(b0) {
  CORTAOPrepassParameters params;
  float4x4 world_to_camera;
};

Texture2D<float> deferred_depth : register(t0);
Texture2D<float4> gbuffer_normal : register(t1);

RWTexture2D<uint> packed_mip0 : register(u0);
RWTexture2D<uint> packed_mip1 : register(u1);
RWTexture2D<uint> packed_mip2 : register(u2);
RWTexture2D<uint> packed_mip3 : register(u3);
RWTexture2D<uint> packed_mip4 : register(u4);
RWTexture2D<uint> packed_mip5 : register(u5);
RWTexture2D<uint> packed_mip6 : register(u6);
RWTexture2D<uint> packed_mip7 : register(u7);

float cortao_loadDepth(int2 pixelCoords) {
  return deferred_depth.Load(int3(pixelCoords, 0));
}

float3 cortao_loadNormal(int2 pixelCoords) {
  float3 normal = gbuffer_normal.Load(int3(pixelCoords, 0)).xyz * 2.0 - 1.0;
  return normalize(mul((float3x3)world_to_camera, normal));
}

void cortao_writePackedData(int2 pixelCoords, int mipLevel, uint data) {
  switch (mipLevel) {
    case 0: packed_mip0[pixelCoords] = data; break;
    case 1: packed_mip1[pixelCoords] = data; break;
    case 2: packed_mip2[pixelCoords] = data; break;
    case 3: packed_mip3[pixelCoords] = data; break;
    case 4: packed_mip4[pixelCoords] = data; break;
    case 5: packed_mip5[pixelCoords] = data; break;
    case 6: packed_mip6[pixelCoords] = data; break;
    case 7: packed_mip7[pixelCoords] = data; break;
  }
}

[numthreads(8, 8, 1)]
void main(uint2 pixelCoords : SV_DispatchThreadID) {
  cortao_prepass(pixelCoords, params);
}
