#ifndef _MATH_HLSL_
#define _MATH_HLSL_

#include "constants.hlsl"

float copysignf(float magnitude, float value)
{
    return value >= 0.0f ? magnitude : -magnitude;
}

// From https://jcgt.org/published/0006/01/01/
float3x3 build_orthonormal_basis(float3 n)
{
    float sign = copysignf(1.0, n.z);
    const float a = -1.0f / (sign + n.z);
    const float b = n.x * n.y * a;
    float3 b1 = float3(1.0f + sign * n.x * n.x * a, sign * b, -sign * n.x);
    float3 b2 = float3(b, sign + n.y * n.y * a, -n.y);

    return float3x3(
        b1.x, b2.x, n.x,
        b1.y, b2.y, n.y,
        b1.z, b2.z, n.z
    );
}

// Samples a direction within a hemisphere oriented along +Z axis with a cosine-weighted distribution 
// See: "Sampling Transformations Zoo" in Ray Tracing Gems by Shirley et al.
float3 sample_hemisphere(float2 urand)
{
    float a = sqrt(urand.x);
	float b = TWO_PI * urand.y;

	float3 result = float3(
		a * cos(b),
		a * sin(b),
		sqrt(1.0f - urand.x));

	//pdf = result.z * ONE_OVER_PI;

	return result;
}

#endif