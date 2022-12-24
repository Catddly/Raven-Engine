#ifndef _GBUFFER_RAY_TRACING_HLSL_
#define _GBUFFER_RAY_TRACING_HLSL_

#include "../math/constants.hlsl"
#include "../defer/gbuffer.hlsl"

#include "gbuffer_ray_payload.hlsl"
#include "ray_cone.hlsl"

struct GBufferHitPoint
{
    PackedGBuffer packed_gbuffer;
    float3 position;
    float  t;
    bool   is_hit;

    static GBufferHitPoint new_miss()
    {
        GBufferHitPoint res;
        res.is_hit = false;
        res.t = FLOAT_MAX;
        res.position = 0.0.xxx;
        return res;
    }
};

struct RayTracingGBuffer
{
    RayDesc sample_ray;
    RayCone ray_cone;
    uint path_index;
    bool back_face_culling;

    static RayTracingGBuffer from_sample_ray(RayDesc sample_ray)
    {
        RayTracingGBuffer res;
        res.sample_ray = sample_ray;
        res.ray_cone = RayCone::from_spread_angle(1.0);
        res.path_index = 0;
        res.back_face_culling = true;
        return res;
    }

    RayTracingGBuffer with_ray_cone(RayCone ray_cone)
    {
        RayTracingGBuffer res = this;
        res.ray_cone = ray_cone;
        return res;
    }

    RayTracingGBuffer with_path_index(uint path_index)
    {
        RayTracingGBuffer res = this;
        res.path_index = path_index;
        return res;
    }

    RayTracingGBuffer with_back_face_culling(bool enable)
    {
        RayTracingGBuffer res = this;
        res.back_face_culling = enable;
        return res;
    }

    GBufferHitPoint trace(RaytracingAccelerationStructure tlas)
    {
        GBufferRayPayload payload = GBufferRayPayload::new_miss();
        payload.ray_cone = this.ray_cone;
        payload.path_index = this.path_index;

        uint flags = 0;
        if (this.back_face_culling)
        {
            flags |= RAY_FLAG_CULL_BACK_FACING_TRIANGLES;
        }

        TraceRay(
            tlas,
            flags,
            0xff, 0, 0, 0,
            this.sample_ray, payload
        );

        GBufferHitPoint res = GBufferHitPoint::new_miss();

        if (payload.is_hit())
        {
            res.packed_gbuffer = payload.packed_gbuffer;
            res.t = payload.t;
            res.position = sample_ray.Origin + sample_ray.Direction * payload.t;
            res.is_hit = true;
        }
        else
        {
            res.t = FLOAT_MAX;
            res.is_hit = false;
        }

        return res;
    }
};

#endif