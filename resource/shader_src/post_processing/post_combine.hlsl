#include "../common/uv.hlsl"
#include "../common/immutable_sampler.hlsl"

#include "../color/tone_mapping.hlsl"
#include "../color/color_space.hlsl"

[[vk::binding(0)]] Texture2D<float4> input_tex;
[[vk::binding(1)]] RWTexture2D<float3> output_tex;
[[vk::binding(2)]] cbuffer _dyn {
    float4 output_tex_size;
    float  post_exposure_mult;
    float  contrast;
    uint   enable_auto_exposure;
    uint   bloom_mip_level;
};
[[vk::binding(3)]] Texture2D<float3> bloom_pyramid;
[[vk::binding(4)]] Texture2D<float3> blur_pyramid_tex;
[[vk::binding(5)]] Texture2D<float3> rev_blur_pyramid_tex;
[[vk::binding(6)]] StructuredBuffer<uint> histogram_buffer;

[numthreads(8, 8, 1)]
void main(uint2 px: SV_DispatchThreadID)
{
    float2 uv = pixel_to_uv(float2(px), output_tex_size.xy);
    float3 color = input_tex[px].rgb;

    [unroll]
    for (uint i = 1; i < bloom_mip_level; ++i)
    {
        color += bloom_pyramid.SampleLevel(sampler_lnce, uv, float(i));
    }

    // if (enable_auto_exposure >= 1)
    // {
    //     const float GLARE_AMOUNT = 0.1;

    //     float2 uv = pixel_to_uv(float2(px), float2(output_tex_size.xy));

    //     float3 glare = rev_blur_pyramid_tex.SampleLevel(sampler_lnce, uv, 0.0);

    //     color = lerp(color, glare, GLARE_AMOUNT);
    //     color = max(color, 0.0);
    // }

    color *= post_exposure_mult; // exposure adjustment

    color = pow(color, contrast);

    // tone mapping
    color = aces_film(color);
 
    // gamma correction
    color = gamma_correction(color, 2.2);

    output_tex[px] = color;
}