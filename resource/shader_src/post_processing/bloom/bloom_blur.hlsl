#include "../../color/luminance.hlsl"
#include "../../common/uv.hlsl"

[[vk::binding(0)]] Texture2D<float3>   input_tex;
[[vk::binding(1)]] RWTexture2D<float3> output_tex;

static const uint NUM_THREADS_X = 64;
static const uint BLUR_KERNEL_RADIUS = 7;
static const uint BLUR_WINDOW_SIZE = (NUM_THREADS_X + BLUR_KERNEL_RADIUS) * 2;

groupshared float3 vblur_color[BLUR_WINDOW_SIZE];

// From kajiya, calculate the weight of gaussian blur
float gaussian_weight(float dst_px, float src_px)
{
    float px_off = (dst_px + 0.5) * 2 - (src_px + 0.5);
    float sigma = BLUR_KERNEL_RADIUS * 0.5;
    return exp(-px_off * px_off / (sigma * sigma));
}

float3 vertical_blur(int2 dst_px, int2 src_px)
{
    float3 res = 0;
    float  weight = 0;

	for (uint y = 0; y <= BLUR_KERNEL_RADIUS * 2; ++y)
    {
        float wt = gaussian_weight(float(dst_px.y), float(src_px.y + y));
		res += input_tex[src_px + int2(0, y)].xyz * wt;
        weight += wt;
	}

	return res / weight;
}

[numthreads(NUM_THREADS_X, 1, 1)]
void main(uint2 px: SV_DispatchThreadID, uint2 px_within_group: SV_GroupThreadID, uint2 group_id: SV_GroupID)
{
    // 1. Vetical Blur
    for (int xfetch = px_within_group.x; xfetch < BLUR_WINDOW_SIZE; xfetch += NUM_THREADS_X)
    {
        int2 src_px = group_id * int2(NUM_THREADS_X * 2, 2) + int2(xfetch - BLUR_KERNEL_RADIUS, -BLUR_KERNEL_RADIUS);
        float3 vblured_pixel = vertical_blur(px, src_px);

        vblur_color[xfetch] = vblured_pixel;
    }

    GroupMemoryBarrierWithGroupSync();

    // 2. Horizontal Blur
    float3 res = 0;
    float  weight = 0;

	for (uint x = 0; x <= BLUR_KERNEL_RADIUS * 2; ++x)
    {
        float wt = gaussian_weight(float(px.x), float(px.x * 2 + x - BLUR_KERNEL_RADIUS));
		res += vblur_color[px_within_group.x * 2 + x] * wt;
        weight += wt;
	}
	res /= weight;

    output_tex[px] = res;
}