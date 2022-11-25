#include "../common/immutable_sampler.hlsl"
#include "../common/uv.hlsl"

[[vk::binding(0)]] Texture2D<float3>   input_tail_tex;
[[vk::binding(1)]] Texture2D<float3>   input_tex;
[[vk::binding(2)]] RWTexture2D<float3> output_tex;
[[vk::binding(3)]] cbuffer _dyn {
    uint  out_width;
    uint  out_height;
    float weight;
};

[numthreads(8, 8, 1)]
void main(uint2 px: SV_DispatchThreadID)
{
    const float3 input_tail_color = input_tail_tex[px];

    float3 self_color = 0.0.xxx;

    // TODO: do a small Gaussian blur instead of this nonsense
    //const uint k = 1;

    // a 3x3 kernel
    for (uint y = -1; y <= 1; y++)
    {
        for (uint x = -1; x <= 1; x++)
        {
            uint2 sample_px = px + uint2(x, y);
            if (sample_px.x < 0)
                sample_px.x = 0;
            else if (sample_px.x >= out_width)
                sample_px.x = out_width - 1;

            if (sample_px.y < 0)
                sample_px.y = 0;
            else if (sample_px.y >= out_height)
                sample_px.y = out_height - 1;

            const float2 uv = pixel_to_uv(float2(sample_px), float2(out_width, out_height));

            float3 sample_color = input_tex.SampleLevel(sampler_lnce, uv, 0.0);
            self_color += sample_color;
        }
    }

    self_color /= 9.0;

    const float exponential_falloff = 0.6;

    // BUG: when `self_weight` is 1.0, the `w` here should be 1.0, not `exponential_falloff`
    output_tex[px] = lerp(self_color, input_tail_color, weight * exponential_falloff);
}