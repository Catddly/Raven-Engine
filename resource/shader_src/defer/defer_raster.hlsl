#include "../common/frame_constants.hlsl"
#include "../common/bindless_resources.hlsl"
#include "../common/material.hlsl"

#include "gbuffer.hlsl"

// TODO: maybe use some batch buffer
[[vk::push_constant]]
struct {
    uint mesh_index;
    uint instance_index;
} push_constants;

[[vk::binding(0)]] StructuredBuffer<float3x4> instance_transforms_dyn; // dynamic read-only storage buffer

struct VsOut {
	float4 position: SV_Position;
    [[vk::location(0)]] float4 color: TEXCOORD0;
    [[vk::location(1)]] float2 uv: TEXCOORD1;
    [[vk::location(2)]] float3 normal: TEXCOORD2;
    [[vk::location(3)]] nointerpolation uint material_id: TEXCOORD3;
    [[vk::location(4)]] float3 tangent: TEXCOORD4;
    [[vk::location(5)]] float3 bitangent: TEXCOORD5;

    [[vk::location(6)]] float3 pos_vs: TEXCOORD6;
};

VsOut vs_main(uint vid: SV_VertexID, uint iid: SV_InstanceID) {
    VsOut vsout;

    CameraMatrices cam = frame_constants_dyn.camera_matrices;

    // get mesh offset data
    const Mesh mesh = meshes[push_constants.mesh_index];

    PackedVertex packed_vertex = PackedVertex(asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.vertex_offset)));
    Vertex vertex = packed_vertex.unpack();

    float4 color = asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.color_offset));
    float4 tangent = asfloat(draw_datas.Load4(vid * sizeof(float4) + mesh.tangent_offset));
    float2 uv = asfloat(draw_datas.Load2(vid * sizeof(float2) + mesh.uv_offset));
    uint material_id = draw_datas.Load(vid * sizeof(uint) + mesh.mat_id_offset);

    float3x4 transform = instance_transforms_dyn[push_constants.instance_index];
    float3 ws_pos = mul(transform, float4(vertex.position, 1.0));
    
    float4 vs_pos = mul(cam.world_to_view, float4(ws_pos, 1.0));
    float4 cs_pos = mul(cam.view_to_clip, vs_pos);

    vsout.position = cs_pos;
    vsout.color = color;
    vsout.uv = uv;
    vsout.normal = vertex.normal;
    vsout.material_id = material_id;
    vsout.tangent = tangent.xyz;
    vsout.bitangent = normalize(cross(vertex.normal, vsout.tangent) * tangent.w);

    // normalize in homogeneous coordinate
    vsout.pos_vs = vs_pos.xyz / vs_pos.w;

    return vsout;
}

struct PsIn {
    [[vk::location(0)]] float4 color: TEXCOORD0;
    [[vk::location(1)]] float2 uv: TEXCOORD1;
    [[vk::location(2)]] float3 normal: TEXCOORD2;
    [[vk::location(3)]] nointerpolation uint material_id: TEXCOORD3;
    [[vk::location(4)]] float3 tangent: TEXCOORD4;
    [[vk::location(5)]] float3 bitangent: TEXCOORD5;

    [[vk::location(6)]] float3 pos_vs: TEXCOORD6;
};

struct PsOut {
    float4 gbuffer: SV_TARGET0;
    float3 geometric_normal: SV_TARGET1;
};

PsOut ps_main(PsIn ps) {
    const Mesh mesh = meshes[push_constants.mesh_index];

    Material mat = draw_datas.Load<Material>(ps.material_id * sizeof(Material) + mesh.mat_data_offset);
    float3 base_color = float4(mat.base_color).rgb;

    float3 normal_ws = normalize(mul(instance_transforms_dyn[push_constants.instance_index], float4(ps.normal, 0.0)));

    // derive geometric normal from view space pos
    // TODO: why not derive it using world space pos?
    float3 dx = ddx(ps.pos_vs);
    float3 dy = ddy(ps.pos_vs);
    // in right hand coordinate system, cross(ddy, ddx), not (ddx, ddy)
    float3 geometric_normal_vs = normalize(cross(dy, dx));

    GBuffer gbuffer = GBuffer::zero();
    gbuffer.albedo = base_color * ps.color.rgb;
    gbuffer.normal = normal_ws;
    gbuffer.metallic = mat.metallic;
    gbuffer.roughness = mat.roughness;

    PsOut psout;
    psout.gbuffer = asfloat(gbuffer.pack().data);
    // store the geometric view space normal
    psout.geometric_normal = geometric_normal_vs * 0.5 + 0.5;
    return psout;
}
