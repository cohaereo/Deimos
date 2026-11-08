#include "include/screen_space_vs.hlsl"
#include "include/view.hlsl"
#include "include/frame.hlsl"

static const uint SAMPLE_NUM = 9;
static const float2 POISSON_SAMPLES[SAMPLE_NUM] = {
  float2(0.0f, 0.0f),
  float2(-0.45570817558838717f, -0.8078111571344746f),
  float2(0.8960089702138108f, -0.3569415776898711f),
  float2(-0.19807511750350795f, 0.9210419317460738f),
  float2(0.2595217206107728f, -0.8090512416351279f),
  float2(0.5972218724881266f, 0.7789149385216658f),
  float2(-0.8088925785649114f, 0.45309126356924434f),
  float2(-0.9167903846266768f, -0.21303266118076764f),
  float2(0.6772830402709191f, 0.18318818607984763f),
};

#define NUM_CASCADES 4
cbuffer scope_shadowmap : register(b0) {
  float4x4 world_to_cascade_projective[NUM_CASCADES];
};

// Textures
Texture2D<float4> gbuffer_normal : register(t0);
Texture2D<float4> gbuffer_third : register(t1);
Texture2D<float> deferred_depth : register(t2);
Texture2D<float4> hdri : register(t3);

Texture2D<float> cascade_0 : register(t10);
Texture2D<float> cascade_1 : register(t11);
Texture2D<float> cascade_2 : register(t12);
Texture2D<float> cascade_3 : register(t13);

SamplerState samplerState : register(s0);

float InterleavedGradientNoise(float2 position_screen) {
  float3 magic = float3(0.06711056f, 0.00583715f, 52.9829189f);
  return frac(magic.z * frac(dot(position_screen, magic.xy)));
}

float2 GetTexelSize(Texture2D<float> tex) {
  uint width, height;
  tex.GetDimensions(width, height);
  return float2(1.0f / width, 1.0f / height);
}

Texture2D<float> GetCascadeTex(uint cascade) {
  switch (cascade) {
  case 0:
    return cascade_0;
  case 1:
    return cascade_1;
  case 2:
    return cascade_2;
  case 3:
    return cascade_3;
  default:
    return cascade_0;
  }
}

float SampleShadow(float3 world_pos, float2 screen_pos, uint cascade) {
  float4 posLightSpace =
      mul(world_to_cascade_projective[cascade], float4(world_pos.xyz, 1.0));
  posLightSpace /= posLightSpace.w; // Normalize homogeneous coordinates

  float2 cascadeUv;
  cascadeUv.x = posLightSpace.x * 0.5 + 0.5;
  cascadeUv.y = 1.0 - (posLightSpace.y * 0.5 + 0.5);

  if (cascadeUv.x < 0.0 || cascadeUv.x > 1.0 || cascadeUv.y < 0.0 ||
      cascadeUv.y > 1.0)
    return 1.0;

  if (posLightSpace.z < 0.0 || posLightSpace.z > 1.0)
    return 1.0;

  float bias = 0.0001;
  Texture2D<float> shadowmap = GetCascadeTex(cascade);
  float2 texelSize = GetTexelSize(shadowmap);

  float filterSpread = 2.5;

  float randomAngle =
      InterleavedGradientNoise(screen_pos) * 6.28318530718; // 2 * PI
  float s = sin(randomAngle);
  float c = cos(randomAngle);

  float2x2 rot = float2x2(c, -s, s, c);

  float shadow = 0.0;

  [unroll] // Optional: unrolling can help performance for small loops
  for (int i = 0; i < SAMPLE_NUM; ++i) {
    float2 rotatedOffset = mul(rot, POISSON_SAMPLES[i]);
    float2 offset = rotatedOffset * texelSize * filterSpread;

    float shadowDepth = shadowmap.Sample(samplerState, cascadeUv + offset).x;
    shadow += (posLightSpace.z - bias) > shadowDepth ? 0.0 : 1.0;
  }
  shadow /= float(SAMPLE_NUM);

  return shadow;
}

static const float PI = 3.14159265359f;
static const float MAX_MIP = 10.0f;

static const float3 SH[9] = {
  float3(7.183077767f, 7.754664474f, 9.071860993f),
  float3(2.242091441f, 2.352030814f, 2.41075356f),
  float3(4.197033062f, 4.343360751f, 4.452158063f),
  float3(3.326343672f, 3.522275553f, 3.690136237f),
  float3(0.9821130764f, 1.013653188f, 1.011707367f),
  float3(1.168266813f, 1.187513731f, 1.126587214f),
  float3(0.7326557854f, 0.717592467f, 0.6322692946f),
  float3(1.715464074f, 1.752283709f, 1.68030064f),
  float3(0.3366667255f, 0.3572046372f, 0.3679988666f),
};

float3 EvaluateSH(float3 n) {
  float x = n.x;
  float y = n.y;
  float z = n.z;

  float basis[9] = { 0.282095f,
                     0.488603f * y,
                     0.488603f * z,
                     0.488603f * x,
                     1.092548f * x * y,
                     1.092548f * y * z,
                     0.315392f * (3.0f * z * z - 1.0f),
                     1.092548f * x * z,
                     0.546274f * (x * x - y * y) };

  float3 result = 0.0f;

  [unroll]
  for (int i = 0; i < 9; i++) {
    result += SH[i] * basis[i];
  }

  return max(result, 0.0f);
}

float2 DirToEquirect(float3 dir) {
  float phi = atan2(dir.y, dir.x);
  float theta = asin(clamp(dir.z, -1.0f, 1.0f));
  return float2(phi / (2.0f * PI) + 0.5f, 0.5f - theta / PI);
}

float3 SampleHDRI(float3 dir, float mip) {
  return hdri.SampleLevel(samplerState, DirToEquirect(dir), mip).rgb * 1.5f;
}

float2 EnvBRDFApprox(float NoV, float roughness) {
  const float4 c0 = float4(-1.0f, -0.0275f, -0.572f, 0.022f);
  const float4 c1 = float4(1.0f, 0.0425f, 1.040f, -0.040f);
  float4 r = roughness * c0 + c1;
  float a004 = min(r.x * r.x, exp2(-9.28f * NoV)) * r.x + r.y;
  return saturate(float2(-1.04f, 1.04f) * a004 + r.zw);
}

float3 F_SchlickRoughness(float cosTheta, float3 F0, float roughness) {
  float3 hi =
      max(float3(1.0f - roughness, 1.0f - roughness, 1.0f - roughness), F0);
  return F0 + (hi - F0) * pow(saturate(1.0f - cosTheta), 5.0f);
}

float remap(float value, float min, float max) {
  return saturate((value - min) / (max - min));
}

void mainPS(VSOutput input, out float4 light_diffuse: SV_TARGET0,
            out float4 light_specular: SV_TARGET1) {

  float depth = deferred_depth.Sample(samplerState, input.uv);
  if (depth == 0.0f) {
    discard;
  }

  float4 cameraPos =
      mul(target_pixel_to_camera, float4(input.pos.xy, depth, 1.0f));
  cameraPos /= cameraPos.w;
  float3 worldPos = mul(camera_to_world, cameraPos).xyz;

  float4 rt1 = gbuffer_normal.Sample(samplerState, input.uv);
  float4 rt2 = gbuffer_third.Sample(samplerState, input.uv);

  float3 rawNormal = rt1.xyz * 2.0f - 1.0f;
  float3 N = normalize(rawNormal);

  float smoothness = saturate(length(rawNormal) * 4.0f - 3.0f);
  float roughness = 1.0f - smoothness;

  float metallic = rt2.r;
  float textureEmissive = saturate(rt2.g * 2.0f - 1.0f);
  float textureAo = saturate(rt2.g * 2.0f);

  float3 V = normalize(camera_position - worldPos);
  float3 R = reflect(-V, N);

  float NoV = max(dot(N, V), 1e-4f);

  float3 F0 =
      lerp(float3(0.04f, 0.04f, 0.04f), float3(1.0f, 1.0f, 1.0f), metallic);

  float3 irradiance = EvaluateSH(N) * 0.4f;

  float3 kS = F_SchlickRoughness(NoV, F0, roughness);
  float3 kD = (1.0f - kS) * (1.0f - metallic);

  float3 diffuse = kD * irradiance * textureAo;

  float specularMip = roughness * MAX_MIP;
  float3 LD = SampleHDRI(R, specularMip);
  float2 dfg = EnvBRDFApprox(NoV, roughness);

  float3 specular = LD * (F0 * dfg.x + dfg.y) * textureAo;

  float shadow = 1.0;
  [unroll]
  for (int i = 0; i < NUM_CASCADES; i++) {
    shadow *= SampleShadow(worldPos, input.pos.xy, i);
  }

  float shadowFactor = 0.97f;
  shadow = lerp(1.0f, shadow, shadowFactor);

  light_diffuse = float4(diffuse * shadow * exposure_scale, 1.0f);
  light_specular = float4(specular * shadow * exposure_scale, 1.0f);
}
