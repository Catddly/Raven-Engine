#ifndef _MATH_HLSL_
#define _MATH_HLSL_

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

#endif