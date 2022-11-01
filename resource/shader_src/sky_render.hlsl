#include "common/uv.hlsl"
#include "common/float_precision.hlsl"
#include "common/immutable_sampler.hlsl"
#include "ray_tracing/camera_ray.hlsl"

[[vk::binding(0)]] Texture2D<float> depth_tex;
[[vk::binding(1)]] TextureCube      cube_map;
[[vk::binding(2)]] RWTexture2D<float4> output_tex;
[[vk::binding(3)]] cbuffer cb_dyn {
    uint render_res_width;
    uint render_res_height;
};

[numthreads(8, 8, 1)]
void main(in uint2 px: SV_DispatchThreadID) {
    const float depth = depth_tex[px];

    [branch]
    if (depth - 0.0 < FLOAT_EPSILON)
    {
        float2 resolution = float2(render_res_width, render_res_height);
        float2 uv = pixel_to_uv(float2(px), resolution);

        CameraRayContext cam_ctx = CameraRayContext::from_screen_uv(uv);
        float3 direction = cam_ctx.get_direction_ws();

        float4 pixel = cube_map.SampleLevel(sampler_llce, direction, 0);

        output_tex[px] = float4(pixel.rgb, 1.0);
    }
}