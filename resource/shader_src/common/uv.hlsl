#ifndef _UV_HLSL_
#define _UV_HLSL_

float2 pixel_to_uv(float2 pixel, float2 resolution)
{
    // shift 0.5 here, because we want to use this uv to calculate the NDC coordinates of some pixel.
    return (pixel + 0.5.xx) / resolution;
}

// vulkan only
float2 uv_to_clip(float2 uv)
{
    return (uv - 0.5.xx) * float2(2.0, -2.0);
}

#endif