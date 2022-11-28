//#include "../../common/immutable_sampler.hlsl"

[[vk::binding(0)]] Texture2D<float3>   input_tex;
[[vk::binding(1)]] RWTexture2D<float3> output_tex;

static const uint EROSION_KERNEL_RADIUS = 1;
//static const uint SHARED_DATA_SIZE = (8 + EROSION_KERNEL_RADIUS * 2) * (8 + EROSION_KERNEL_RADIUS * 2);

[numthreads(8, 8, 1)]
void main(uint2 px: SV_DispatchThreadID)
{
    for (int y = -1; y <= 1; ++y)
    {
        for (int x = -1; x <= 1; ++x)
        {
            int2 px_offset = int2(px) + int2(x, y);
            float3 color = input_tex[px_offset];

            if (dot(color, color) < 0.01)
            {
                output_tex[px] = 0.0.xxx;
                return;
            }
        }
    }

    output_tex[px] = input_tex[px];
}