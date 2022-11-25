#include "../../color/luminance.hlsl"
#include "../../common/uv.hlsl"
#include "../../common/immutable_sampler.hlsl"

[[vk::binding(0)]] Texture2D<float4>   input_tex;
[[vk::binding(1)]] RWTexture2D<float3> output_tex;
[[vk::binding(2)]] cbuffer _dyn {
    uint2 output_tex_size;
    float lum_threshold;
};

[numthreads(8, 8, 1)]
void main(uint2 px: SV_DispatchThreadID)
{
    float2 uv = pixel_to_uv(float2(px), float2(output_tex_size));

    float3 input_color = input_tex.SampleLevel(sampler_lnce, uv, 0.0).rgb;
    float lum = rgb_color_to_luminance(input_color);

    if (lum > (lum_threshold + 0.1))
    {
        output_tex[px] = input_color;
    }
    else
    {
        output_tex[px] = float3(0.0, 0.0, 0.0);
    }
}