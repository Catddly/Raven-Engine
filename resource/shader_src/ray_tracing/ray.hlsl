#ifndef _RAY_HLSL_
#define _RAY_HLSL_

RayDesc new_ray(float3 origin, float3 direction, float tmin, float tmax)
{
    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = direction;
    ray.TMin = tmin;
    ray.TMax = tmax;
    return ray;
}

#endif