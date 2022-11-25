#ifndef _COLOR_SPACE_HLSL_
#define _COLOR_SPACE_HLSL_

float3 srgb_to_linear(float3 color)
{
    return pow(color, 2.2);
}

float3 linear_to_srgb(float3 color)
{
    return pow(color, 1.0 / 2.2);
}

float3 gamma_correction(float3 color, float gamma)
{
    return pow(color, 1.0 / gamma);
}

#endif