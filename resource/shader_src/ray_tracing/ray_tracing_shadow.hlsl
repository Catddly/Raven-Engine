#ifndef _RAY_TRACING_SHADOW_HLSL_
#define _RAY_TRACING_SHADOW_HLSL_

#include "../math/constants.hlsl"

#include "ray.hlsl"

struct ShadowRayPayload
{
    bool is_shadowed;

    // if we hit something here, means that position is blocked by some object,
    // so it is shadowed.
    static ShadowRayPayload new_shadowed()
    {
        ShadowRayPayload res;
        res.is_shadowed = true;
        return res;
    }
};

// Return if this position is shadowed or not.
bool ray_trace_shadow_directional(RaytracingAccelerationStructure tlas, float3 hit_pos, float3 direction)
{
    const float3 flip_dir = -direction;
    RayDesc shadow_ray = new_ray(
        hit_pos,
        normalize(flip_dir),
        1e-3,
        FLOAT_MAX
    );
    ShadowRayPayload payload = ShadowRayPayload::new_shadowed();

    TraceRay(
        tlas,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH | RAY_FLAG_SKIP_CLOSEST_HIT_SHADER,
        0xff, 0, 0, 1, // use second miss shader
        shadow_ray, payload
    );

    return payload.is_shadowed;
}

#endif