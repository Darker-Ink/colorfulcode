#include "Common.hlsli"
#define TILE_SIZE 8

// Blurs an image horizontally with a compute shader.
cbuffer BlurParams : register(b0)
{
    float4x4 ViewProj;
    float2 TexelSize;
    uint Radius;
};

Texture2D<float4> InputTex : register(t0);
RWTexture2D<float4> OutputTex : register(u0);
SamplerState LinearClamp : register(s0);

struct VSOutput
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

float gaussian(float x, float sigma)
{
    return exp(-(x * x) / (2.0f * sigma * sigma));
}

[numthreads(TILE_SIZE, TILE_SIZE, 1)]
void CSMain(uint3 id : SV_DispatchThreadID)
{
    float4 sum = 0;
    float weights = 0.0;
    [unroll]
    for (int i = -4; i <= 4; ++i)
    {
        float w = gaussian(i, 2.5);
        sum += InputTex.Load(int3(id.xy + int2(i, 0), 0)) * w;
        weights += w;
    }
    OutputTex[id.xy] = saturate(sum / weights);
}

float4 PSMain(VSOutput input) : SV_Target
{
    float3 color = InputTex.Sample(LinearClamp, input.uv).rgb;
    return float4(lerp(color, 1.0.xxx, 0.1), 1.0);
}
