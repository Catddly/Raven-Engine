#include "../../ray_tracing/gbuffer_ray_payload.hlsl"

// We already assume that the ray is miss by new_miss().
// So this shader do nothing for now.
[shader("miss")]
void main(inout GBufferRayPayload payload: SV_RayPayload) {}