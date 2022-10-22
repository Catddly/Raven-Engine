#include "gbuffer.hlsl"
#include "../common/float_precision.hlsl"

[[vk::binding(0)]] Texture2D<float4> gbuffer_tex;
[[vk::binding(1)]] Texture2D<float> depth_tex;
[[vk::binding(2)]] RWTexture2D<float4> output_tex;

[numthreads(8, 8, 1)]
void main(in uint2 px: SV_DispatchThreadID) {
    const float depth = depth_tex[px];
    if (depth - 0.0 < FLOAT_EPSILON)
    {
        output_tex[px] = float4(0.0, 0.0, 0.0, 0.0);
        return;
    }

    GBuffer gbuffer = PackedGBuffer::from_uint4(asuint(gbuffer_tex[px])).unpack();

    output_tex[px] = float4(gbuffer.albedo, 1.0);
}