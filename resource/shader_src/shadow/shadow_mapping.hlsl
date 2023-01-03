#include "../common/bindless_resources.hlsl"

[[vk::push_constant]]
struct {
    uint mesh_index;
    uint instance_index; // this is instance index for now
} push_constants;

[[vk::binding(0)]] StructuredBuffer<row_major float4x4> light_transforms_dyn;  // dynamic read-only storage buffer
[[vk::binding(1)]] StructuredBuffer<row_major float3x4> object_transforms_dyn; // dynamic read-only storage buffer

float4 vs_main(uint vid: SV_VertexID) : SV_Position
{
    // get mesh offset data
    const Mesh mesh = meshes[push_constants.mesh_index];

    PackedVertex packed_vertex = PackedVertex(asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.vertex_offset)));
    Vertex vertex = packed_vertex.unpack();

    float3 vertex_pos_ws = mul(object_transforms_dyn[push_constants.instance_index], float4(vertex.position, 1.0));

    return mul(light_transforms_dyn[0], float4(vertex_pos_ws, 1.0));
}

// Only draw objects in shadow map
void ps_main() {}