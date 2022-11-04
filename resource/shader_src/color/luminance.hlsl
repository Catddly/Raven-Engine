#ifndef _LUMINANCE_HLSL_
#define _LUMINANCE_HLSL_

float rgb_color_to_luminance(float3 rgb)
{
    return dot(rgb, float3(0.2126, 0.7152, 0.0722));
}

#endif