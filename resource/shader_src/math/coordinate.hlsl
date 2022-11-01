#ifndef _COORDINATE_HLSL_
#define _COORDINATE_HLSL_

// theta is the horizontal angle in radians
// phi is the vertical angle in radians
float3 spherical_to_cartesian_unit_sphere(float theta, float phi)
{
    return float3(
        sin(phi) * cos(theta),
        sin(phi) * sin(theta),
        cos(phi)
    );
}

#endif