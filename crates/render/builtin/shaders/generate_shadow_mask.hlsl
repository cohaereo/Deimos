#include "include/screen_space_vs.hlsl"
#include "include/view.hlsl"

static const uint SAMPLE_NUM = 16;
static const float2 POISSON_SAMPLES[SAMPLE_NUM] = {
    float2(0.176776695f, 0.000000000f),
    float2(-0.225772188f, 0.206825818f),
    float2(0.034558052f, -0.393771179f),
    float2(0.284571220f, 0.371172764f),
    float2(-0.522223187f, -0.092373929f),
    float2(0.494695392f, -0.314684715f),
    float2(-0.165465927f, 0.615525001f),
    float2(-0.315561468f, -0.607594404f),
    float2(0.684642162f, 0.250030219f),
    float2(-0.712256086f, 0.294008958f),
    float2(0.343354499f, -0.733728620f),
    float2(0.253730241f, 0.808931990f),
    float2(-0.764745892f, -0.443185876f),
    float2(0.897133984f, -0.197232390f),
    float2(-0.547506905f, 0.778772232f),
    float2(-0.126486773f, -0.976089697f),
};

#define NUM_CASCADES 4
cbuffer scope_shadowmap : register(b0) {
    float4x4 world_to_cascade_projective[NUM_CASCADES];
    float4 cascade_splits;
    float4 cascade_texel_bias;
    float4 sun_direction;
};

Texture2D<float> deferred_depth : register(t0);
Texture2D<float4> gbuffer_normal : register(t1);

Texture2D<float> cascade_0 : register(t10);
Texture2D<float> cascade_1 : register(t11);
Texture2D<float> cascade_2 : register(t12);
Texture2D<float> cascade_3 : register(t13);

SamplerState samplerState : register(s0);

float InterleavedGradientNoise(float2 position_screen) {
    float3 magic = float3(0.06711056f, 0.00583715f, 52.9829189f);
    return frac(magic.z * frac(dot(position_screen, magic.xy)));
}

uint SelectCascade(float view_depth) {
    if (view_depth > cascade_splits.z) return 3;
    if (view_depth > cascade_splits.y) return 2;
    if (view_depth > cascade_splits.x) return 1;
    return 0;
}

float GetCascadeBias(uint cascade) {
    if (cascade == 0) return cascade_texel_bias.x;
    if (cascade == 1) return cascade_texel_bias.y;
    if (cascade == 2) return cascade_texel_bias.z;
    return cascade_texel_bias.w;
}

float GetCascadeFar(uint cascade) {
    if (cascade == 0) return cascade_splits.x;
    if (cascade == 1) return cascade_splits.y;
    if (cascade == 2) return cascade_splits.z;
    return cascade_splits.w;
}

// Can't pass texture handles to functions, so we're using a macro
#define SAMPLE_CASCADE(TEX)                                                     \
    {                                                                           \
        int2 max_coord = int2(width, height) - 1;                               \
        float2 resolution = float2(width, height);                              \
        float ref_depth = posLightSpace.z - bias;                               \
        [unroll]                                                                \
        for (int i = 0; i < SAMPLE_NUM; ++i) {                                  \
            float2 rotatedOffset = mul(rot, POISSON_SAMPLES[i]) * filterSpread; \
            float2 texel_pos = cascadeUv * resolution + rotatedOffset - 0.5;    \
            int2 base = int2(floor(texel_pos));                                 \
            float2 f = frac(texel_pos);                                         \
            int2 p00 = min(max(base + int2(0, 0), int2(0, 0)), max_coord);      \
            int2 p10 = min(max(base + int2(1, 0), int2(0, 0)), max_coord);      \
            int2 p01 = min(max(base + int2(0, 1), int2(0, 0)), max_coord);      \
            int2 p11 = min(max(base + int2(1, 1), int2(0, 0)), max_coord);      \
            float d00 = TEX.Load(int3(p00, 0));                                 \
            float d10 = TEX.Load(int3(p10, 0));                                 \
            float d01 = TEX.Load(int3(p01, 0));                                 \
            float d11 = TEX.Load(int3(p11, 0));                                 \
            float s00 = ref_depth > d00 ? 0.0 : 1.0;                            \
            float s10 = ref_depth > d10 ? 0.0 : 1.0;                            \
            float s01 = ref_depth > d01 ? 0.0 : 1.0;                            \
            float s11 = ref_depth > d11 ? 0.0 : 1.0;                            \
            shadow += lerp(lerp(s00, s10, f.x), lerp(s01, s11, f.x), f.y);      \
        }                                                                       \
    }

float SampleShadow(float3 world_pos, float2 screen_pos, float3 normal, uint cascade) {
    float4x4 cascade_matrix = world_to_cascade_projective[cascade];
    float4 posLightSpace = mul(cascade_matrix, float4(world_pos.xyz, 1.0));
    posLightSpace /= posLightSpace.w;

    float2 cascadeUv;
    cascadeUv.x = posLightSpace.x * 0.5 + 0.5;
    cascadeUv.y = 1.0 - (posLightSpace.y * 0.5 + 0.5);

    if (cascadeUv.x < 0.0 || cascadeUv.x > 1.0 || cascadeUv.y < 0.0 || cascadeUv.y > 1.0)
        return 1.0;

    if (posLightSpace.z < 0.0 || posLightSpace.z > 1.0)
        return 1.0;

    uint width, height;
    cascade_0.GetDimensions(width, height);

    float NdotL = saturate(abs(dot(normal, sun_direction.xyz)));
    float slope = sqrt(max(1.0 - NdotL * NdotL, 0.0)) / max(NdotL, 0.05);
    float bias = GetCascadeBias(cascade) * (2.0 + slope);

    float filterSpread = 2.5;

    float randomAngle = InterleavedGradientNoise(screen_pos) * 6.28318530718;
    float s = sin(randomAngle);
    float c = cos(randomAngle);

    float2x2 rot = float2x2(c, -s, s, c);

    float shadow = 0.0;

    if (cascade == 0)
        SAMPLE_CASCADE(cascade_0)
    else if (cascade == 1)
        SAMPLE_CASCADE(cascade_1)
    else if (cascade == 2)
        SAMPLE_CASCADE(cascade_2)
    else
        SAMPLE_CASCADE(cascade_3)

    shadow /= float(SAMPLE_NUM);

    return shadow;
}

float4 mainPS(VSOutput input) : SV_TARGET {
    float depth = deferred_depth.Sample(samplerState, input.uv);
    if (depth == 0.0f)
        discard;

    float4 cameraPos = mul(target_pixel_to_camera, float4(input.pos.xy, depth, 1.0f));
    cameraPos /= cameraPos.w;
    float3 worldPos = mul(camera_to_world, cameraPos).xyz;

    float3 rawNormal = gbuffer_normal.Sample(samplerState, input.uv).xyz * 2.0f - 1.0f;
    float3 normal = normalize(rawNormal);

    uint cascade = SelectCascade(-cameraPos.z);
    float shadow = SampleShadow(worldPos, input.pos.xy, normal, cascade);

    // Blend into the next cascade
    if (cascade < NUM_CASCADES - 1) {
        float view_depth = -cameraPos.z;
        float split = GetCascadeFar(cascade);
        float prev_split = cascade == 0 ? 0.0 : GetCascadeFar(cascade - 1);
        float band = 0.1 * (split - prev_split);
        if (view_depth > split - band) {
            float t = saturate((view_depth - (split - band)) / band);
            float next_shadow = SampleShadow(worldPos, input.pos.xy, normal, cascade + 1);
            shadow = lerp(shadow, next_shadow, t);
        }
    }

    return shadow.xxxx;
}
