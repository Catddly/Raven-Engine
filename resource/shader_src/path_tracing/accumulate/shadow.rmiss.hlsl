#include "../../ray_tracing/ray_tracing_shadow.hlsl"

[shader("miss")]
void main(inout ShadowRayPayload payload: SV_RayPayload)
{
    payload.is_shadowed = false;
}