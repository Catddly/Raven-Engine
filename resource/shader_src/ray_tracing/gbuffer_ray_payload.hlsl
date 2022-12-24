#ifndef _GBUFFER_RAY_PAYLOAD_HLSL_
#define _GBUFFER_RAY_PAYLOAD_HLSL_

#include "../math/constants.hlsl"
#include "../defer/gbuffer.hlsl"

#include "ray_cone.hlsl"

// Ray Tracing payload used in gbuffer path tracing.
struct GBufferRayPayload {
    // use packed gbuffer data here to reduce memory usage of the payload,
    // this will be more efficient if the payload was smaller.
    PackedGBuffer packed_gbuffer;
    RayCone ray_cone;
    float t;
    uint path_index;

    static GBufferRayPayload new_miss()
    {
        GBufferRayPayload res;
        res.ray_cone = RayCone::from_spread_angle(0.0);
        res.t = FLOAT_MAX;
        res.path_index = 0;
        return res;
    }

    bool is_miss()
    {
        return this.t == FLOAT_MAX;
    }

    bool is_hit()
    {
        return !is_miss();
    }
};

#endif