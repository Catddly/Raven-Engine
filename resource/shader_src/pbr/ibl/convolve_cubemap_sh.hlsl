#include "../../common/uv.hlsl"

[[vk::push_constant]]
struct {
    uint render_res;
    uint mip_level;
} push_constants;

[[vk::binding(0)]] Texture2DArray<float4> cube_map;
[[vk::binding(1)]] RWTexture2DArray<float4> convolve_cube_map;

[numthreads(8, 8, 1)]
void main(uint3 px: SV_DispatchThreadID)
{
    uint face = px.z;
    float2 uv = pixel_to_uv(px.xy, float2(push_constants.render_res, push_constants.render_res));
}