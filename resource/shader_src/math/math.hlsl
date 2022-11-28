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

	return result;
}

// Van Der Corput sequence.
// Used to generate hammersley low discrepancy sequence.
float radical_inverse_vdc(uint bits) {
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return float(bits) * 2.3283064365386963e-10; // / 0x100000000
}

// Hammersley low-discrepancy sequences.
// Used in Quasi-Monte Carlo integration. (biased)
// See http://holger.dammertz.org/stuff/notes_HammersleyOnHemisphere.html for details.
float2 hammersley(uint i, uint n) {
    return float2(float(i) / float(n), radical_inverse_vdc(i));
}

// Generate a Spherical Fibonacci Sequences.
// Return a unit direction vector in cartesian.
// See https://stackoverflow.com/questions/9600801/evenly-distributing-n-points-on-a-sphere/26127012#26127012
float3 spherical_fibonacci(uint i, uint n) {
    const float sqrt_5 = 2.2360679774997896964092;
    const float phi = PI * (3.0 - sqrt_5); // golden angle in radians

    float y = 1.0 - (float(i) / float(n - 1)) * 2.0; // y goes from 1 to -1
    float radius = sqrt(1.0 - y * y); // radius at y

    float theta = phi * float(i); // golden angle increment

    float x = cos(theta) * radius;
    float z = sin(theta) * radius;

    return normalize(float3(x, y, z));
}

#endif