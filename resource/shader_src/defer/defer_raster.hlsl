#include "../common/bindless.hlsl"
#include "gbuffer.hlsl"

// TODO: maybe use some batch buffer
[[vk::push_constant]]
struct {
    uint mesh_index;
    uint instance_index;
} push_constants;

[[vk::binding(0, 0)]] StructuredBuffer<float3x4> instance_transforms_dyn; // dynamic read-only storage buffer

struct VsOut {
	float4 position: SV_Position;
    [[vk::location(0)]] float4 color: TEXCOORD0;
    [[vk::location(1)]] float2 uv: TEXCOORD1;
    [[vk::location(2)]] float3 normal: TEXCOORD2;
    [[vk::location(3)]] nointerpolation uint material_id: TEXCOORD3;
    [[vk::location(4)]] float3 tangent: TEXCOORD4;
    [[vk::location(5)]] float3 bitangent: TEXCOORD5;
};

VsOut vs_main(uint vid: SV_VertexID, uint iid: SV_InstanceID) {
    VsOut vsout;

    // get mesh offset data
    const Mesh mesh = meshes[push_constants.mesh_index];

    PackedVertex packed_vertex = PackedVertex(asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.vertex_offset)));
    Vertex vertex = packed_vertex.unpack();

    float4 color = mesh.color_offset != 0
        ? asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.color_offset))
        : 1.0.xxxx;

    float4 tangent = mesh.tangent_offset != 0
        ? asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.tangent_offset))
        : float4(1, 0, 0, 1);

    float2 uv = asfloat(draw_datas.Load2(vid * sizeof(float2) + mesh.uv_offset));

    uint material_id = draw_datas.Load(vid * sizeof(uint) + mesh.mat_id_offset);

    float3x4 transform = instance_transforms_dyn[push_constants.instance_index];
    float3 ws_pos = mul(transform, float4(vertex.position, 1.0));

    vsout.position = float4(ws_pos, 1.0);
    vsout.color = color;
    vsout.uv = uv;
    vsout.normal = vertex.normal;
    vsout.material_id = material_id;
    vsout.tangent = tangent.xyz;
    vsout.bitangent = normalize(cross(vertex.normal, vsout.tangent) * tangent.w);

    return vsout;
}

struct PsIn {
    [[vk::location(0)]] float4 color: TEXCOORD0;
    [[vk::location(1)]] float2 uv: TEXCOORD1;
    [[vk::location(2)]] float3 normal: TEXCOORD2;
    [[vk::location(3)]] nointerpolation uint material_id: TEXCOORD3;
    [[vk::location(4)]] float3 tangent: TEXCOORD4;
    [[vk::location(5)]] float3 bitangent: TEXCOORD5;
};

struct PsOut {
    float4 gbuffer: SV_TARGET0;
    float3 geometric_normal: SV_TARGET1;
};

PsOut ps_main(PsIn ps) {
    GBuffer gbuffer = GBuffer::zero();

    gbuffer.albedo = ps.color.rgb;
    gbuffer.normal = ps.normal; // object space normal
    // TODO: sample from material and texture
    gbuffer.metallic = 0.8;
    gbuffer.roughness = 0.2;

    PackedGBuffer packed_gbuffer = gbuffer.pack();

    PsOut psout;
    psout.gbuffer = packed_gbuffer.data;
    psout.geometric_normal = ps.normal * 0.5 + 0.5;
    return psout;
}
