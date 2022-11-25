#include "../../color/luminance.hlsl"
#include "../../common/uv.hlsl"
#include "../../common/frame_constants.hlsl"

#include "luminance_histogram_common.hlsl"

[[vk::binding(0)]] Texture2D<float4> input_tex;
[[vk::binding(1)]] RWStructuredBuffer<uint> output_buffer;
[[vk::binding(2)]] cbuffer _dyn {
    uint2 input_extent;
};

[numthreads(8, 8, 1)]
void main(uint2 px: SV_DispatchThreadID)
{
    if (any(px >= input_extent))
    {
        return;
    }

    const float3 color = input_tex[px].rgb;
    const float log2_lum = log2(max(rgb_color_to_luminance(color) / frame_constants_dyn.pre_exposure_mult, 1e-20));

    const float t = saturate((log2_lum - LUMINANCE_HISTOGRAM_MIN_LOG2) / (LUMINANCE_HISTOGRAM_MAX_LOG2 - LUMINANCE_HISTOGRAM_MIN_LOG2));
    const uint bin_idx = min(uint(t * LUMINANCE_HISTOGRAM_BIN_COUNT), 255); // clamp to [0, 255]

    // See https://knarkowicz.wordpress.com/2016/01/09/automatic-exposure/
    // Player is more likely to focus on the center of the screen and
    // usually the center of the screen is most important for the viewer and should be well exposed.
    // After calculating the log2 luminance of pixel when have to weighted it by the distance to the screen center.
    const float2 uv = pixel_to_uv(float2(px), float2(input_extent));

    const float2 centered_uv = uv - 0.5.xx;
    const float distance_to_center = length(centered_uv);
    // See the weight curves https://www.desmos.com/calculator/xujxnmxo8z
    // Range of weight is around [0.135, 1.0]
    const float weight = exp(-8 * pow(distance_to_center, 2));
    const uint weighted_count = uint(weight * 256.0);

    InterlockedAdd(output_buffer[bin_idx], weighted_count);
}