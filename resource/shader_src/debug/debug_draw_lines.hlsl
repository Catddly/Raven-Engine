#include "../common/frame_constants.hlsl"
#include "../common/bindless_resources.hlsl"

[[vk::push_constant]]
struct {
    uint aabb_index;
} push_constants;

[[vk::binding(0)]] StructuredBuffer<row_major float4x4> debug_aabb_transforms_dyn; // dynamic read-only storage buffer
[[vk::binding(1)]] ByteAddressBuffer debug_aabb_vb;

struct VsOut {
	float4 out_position: SV_Position;
};

VsOut vs_main(uint vid: SV_VertexID)
{
    CameraFrameConstants cam = frame_constants_dyn.camera_constants;

    float3 vertex_os = asfloat(debug_aabb_vb.Load3(vid * sizeof(float3)));
    float4 vertex_ws = mul(debug_aabb_transforms_dyn[push_constants.aabb_index], float4(vertex_os, 1.0));

    VsOut result;
    result.out_position = mul(cam.view_to_clip, mul(cam.world_to_view, vertex_ws));
    return result;
}

struct PsOut {
    float3 output: SV_TARGET0;
};

PsOut ps_main() {
    PsOut res;
    res.output = float3(1.0, 0.0, 0.0);
    return res;
}