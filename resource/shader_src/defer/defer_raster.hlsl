#include "../common/frame_constants.hlsl"
#include "../common/bindless_resources.hlsl"
#include "../common/material.hlsl"
#include "../common/immutable_sampler.hlsl"
#include "../color/color_space.hlsl"

#include "gbuffer.hlsl"

#define FORCE_NO_TEX_GRAY_MODEL 0

// TODO: maybe use some batch buffer
[[vk::push_constant]]
struct {
    uint mesh_index;
    uint instance_index;
} push_constants;

// float3x4 float4x4 matrix is column major matrix in hlsl by default.
// But in CPU-side we pass the affine matrix data by row major organization.
// Why use row major matrix here?
// Because in assembly implementation of mul() function in hlsl, when the matrix is right-multiply (i.e. the matrix is column matrix)
// In order to access the memory more efficient, hlsl will have transpose operation on the matrix, then do the multiplication row by row.
// But when the matrix is left-multiply (i.e. the matrix is row matrix), hlsl wil not do the transpose operation.
// So put the matrix in the left side of the mul() function will save some instrctions for us.
[[vk::binding(0)]] StructuredBuffer<row_major float3x4> instance_transforms_dyn; // dynamic read-only storage buffer

struct VsOut {
	float4 out_position: SV_Position;
    [[vk::location(0)]] float4 color: TEXCOORD0;
    [[vk::location(1)]] float2 uv: TEXCOORD1;
    [[vk::location(2)]] float3 normal: TEXCOORD2;
    [[vk::location(3)]] nointerpolation uint material_id: TEXCOORD3;
    [[vk::location(4)]] float3 tangent: TEXCOORD4;
    [[vk::location(5)]] float3 bitangent: TEXCOORD5;

    [[vk::location(6)]] float3 pos_vs: TEXCOORD6;
};

VsOut vs_main(uint vid: SV_VertexID, uint iid: SV_InstanceID)
{
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

    vsout.out_position = cs_pos;
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

PsOut ps_main(PsIn ps)
{
    const Mesh mesh = meshes[push_constants.mesh_index];

    Material mat = draw_datas.Load<Material>(ps.material_id * sizeof(Material) + mesh.mat_data_offset);
    
    // TODO: apply uv transform (using Material.texture_transform)

    // Sample albedo map
    float4 albedo_texel = 1.0.xxxx;
    if ((mesh.texture_mask & TEXTURE_MASK_ALBEDO_BIT) != 0)
    {   
        Texture2D albedo_map = bindless_textures[NonUniformResourceIndex(mat.albedo_map)];
        albedo_texel = albedo_map.Sample(sampler_llr, ps.uv);
        if (albedo_texel.a < 0.5) {
            discard;
        }
    }   
    float3 base_color = float4(mat.base_color).rgb;

    // Sample Metallic Rougheness
    float metalness = mat.metalness;
    float roughness = clamp(perceptual_roughness_to_roughness(mat.roughness), 1e-3, 1.0);
    if ((mesh.texture_mask & TEXTURE_MASK_SPECULAR_BIT) != 0)
    {
        Texture2D specular_map = bindless_textures[NonUniformResourceIndex(mat.specular_map)];
        float4 specular_texel = specular_map.Sample(sampler_llr, ps.uv);
        metalness *= specular_texel.z;
        float peceptual_roughness = mat.roughness * specular_texel.y;
        roughness = clamp(perceptual_roughness_to_roughness(peceptual_roughness), 1e-3, 1.0); // In reality, no object is purely smooth
    }

    // Sample normal
    float3 normal_os = ps.normal;

    if ((mesh.texture_mask & TEXTURE_MASK_NORMAL_BIT) != 0)
    {
        Texture2D normal_map = bindless_textures[NonUniformResourceIndex(mat.normal_map)];
        float4 normal_texel = normal_map.Sample(sampler_llr, ps.uv);
        float3 normal_ts = float3(normal_texel.xy * 2.0 - 1.0, 0.0); // remap from [0, 1] to [-1, 1]
        normal_ts.z = sqrt(max(0.001, 1.0 - dot(normal_ts.xy, normal_ts.xy))); // normal in normal map is already normalized

        float3x3 tbn_matrix = float3x3(ps.tangent, ps.bitangent, ps.normal);
        normal_os = mul(normal_ts, tbn_matrix);
    }

    float3 normal_ws = normalize(mul(instance_transforms_dyn[push_constants.instance_index], float4(normal_os, 0.0)));

    // derive geometric normal from view space pos
    // TODO: why not derive it using world space pos?
    float3 dx = ddx(ps.pos_vs);
    float3 dy = ddy(ps.pos_vs);
    // in right hand coordinate system, cross(ddy, ddx), not (ddx, ddy)
    float3 geometric_normal_vs = normalize(cross(dy, dx));

    CameraMatrices cam = frame_constants_dyn.camera_matrices;
    float3 geometric_normal_ws = mul(cam.view_to_world, float4(geometric_normal_vs, 0.0)).rgb;

    // geometric normal and shading normal is pointing the opposite direction, fix it.
    if (dot(geometric_normal_ws, normal_ws) < 0)
    {
        normal_ws *= -1; // simply flipping opposite
    }

    // Texture2D emissive_map = bindless_textures[NonUniformResourceIndex(mat.emissive_map)];
    // float4 emissive_texel = emissive_map.Sample(sampler_llr, ps.uv);

    GBuffer gbuffer = GBuffer::zero();
    
#if FORCE_NO_TEX_GRAY_MODEL
    gbuffer.albedo = 0.5.xxx;
#else
    gbuffer.albedo = base_color * ps.color.rgb * albedo_texel.rgb;
#endif
    gbuffer.normal = normal_ws;

#if FORCE_NO_TEX_GRAY_MODEL
    gbuffer.metalness = 0.0;
    gbuffer.roughness = 0.5;
#else
    gbuffer.metalness = metalness;
    gbuffer.roughness = roughness;
#endif

    PsOut psout;
    psout.gbuffer = asfloat(gbuffer.pack().data);
    // store the geometric view space normal
    psout.geometric_normal = geometric_normal_vs * 0.5 + 0.5;
    return psout;
}
