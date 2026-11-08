#include "include/screen_space_vs.hlsl"
#include "include/view.hlsl"

// Textures
Texture2D<float4> gbuffer_normal : register(t0);
Texture2D<float4> gbuffer_third : register(t1);
Texture2D<float> deferred_depth : register(t2);
Texture2D<float4> hdri : register(t3);
Texture2D<float> shadow_mask : register(t4);

SamplerState samplerState : register(s0);

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

  float shadow = shadow_mask.Sample(samplerState, input.uv);
  float shadowFactor = 0.93f;
  shadow = lerp(1.0f, shadow, shadowFactor);

  light_diffuse = float4(diffuse * shadow * exposure_scale, 1.0f);
  light_specular = float4(specular * shadow * exposure_scale, 1.0f);
}
