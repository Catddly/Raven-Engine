#include "../../common/bindless_resources.hlsl"
#include "../../common/immutable_sampler.hlsl"

#include "../../defer/gbuffer.hlsl"

#include "../../ray_tracing/ray_cone.hlsl"
#include "../../ray_tracing/gbuffer_ray_payload.hlsl"

// reference paper:
// [1] https://media.contentapi.ea.com/content/dam/ea/seed/presentations/2019-ray-tracing-gems-chapter-20-akenine-moller-et-al.pdf

[[vk::binding(0)]] RaytracingAccelerationStructure tlas;

struct RayHitAttrib
{
    float2 bary;
};

uint3 load_indices_from_mesh(Mesh mesh)
{
    const uint prim_index = PrimitiveIndex();
    return uint3(
        draw_datas.Load((prim_index * 3 + 0) * sizeof(uint) + mesh.index_offset),
        draw_datas.Load((prim_index * 3 + 1) * sizeof(uint) + mesh.index_offset),
        draw_datas.Load((prim_index * 3 + 2) * sizeof(uint) + mesh.index_offset)
    );
}

struct Vertices
{
    Vertex v0;
    Vertex v1;
    Vertex v2;
};

Vertices load_vertices_from_mesh(Mesh mesh, uint3 indices)
{
    Vertices res;
    res.v0 = PackedVertex(asfloat(draw_datas.Load4(indices.x * sizeof(float4) + mesh.vertex_offset))).unpack();
    res.v1 = PackedVertex(asfloat(draw_datas.Load4(indices.y * sizeof(float4) + mesh.vertex_offset))).unpack();
    res.v2 = PackedVertex(asfloat(draw_datas.Load4(indices.z * sizeof(float4) + mesh.vertex_offset))).unpack();
    return res;
}

float4 load_color_from_mesh(Mesh mesh, uint3 indices, float3 bary)
{
    float4 c0 = asfloat(draw_datas.Load4(indices.x * sizeof(float4) + mesh.color_offset));
    float4 c1 = asfloat(draw_datas.Load4(indices.y * sizeof(float4) + mesh.color_offset));
    float4 c2 = asfloat(draw_datas.Load4(indices.z * sizeof(float4) + mesh.color_offset));
    return c0 * bary.x + c1 * bary.y + c2 * bary.z;
}

float2 load_uv_from_mesh(Mesh mesh, uint3 indices, float3 bary)
{
    float2 uv0 = asfloat(draw_datas.Load2(indices.x * sizeof(float2) + mesh.uv_offset));
    float2 uv1 = asfloat(draw_datas.Load2(indices.y * sizeof(float2) + mesh.uv_offset));
    float2 uv2 = asfloat(draw_datas.Load2(indices.z * sizeof(float2) + mesh.uv_offset));
    return uv0 * bary.x + uv1 * bary.y + uv2 * bary.z;
}

// equation 4. in Paper [1]
float twice_uv_area(float2 uv0, float2 uv1, float2 uv2)
{
    return abs((uv1.x - uv0.x) * (uv2.y - uv0.y) - (uv2.x - uv0.x) * (uv1.y - uv0.y));
}

// equation 5. in Paper [1]
float twice_triangle_area(float3 p0, float3 p1, float3 p2)
{
    return length(cross(p1 - p0, p2 - p0));
}

[shader("closesthit")]
void main(inout GBufferRayPayload payload: SV_RayPayload, in RayHitAttrib attrib: SV_IntersectionAttributes)
{
    // 1. Get vertex information from hit triangle.
    float3 barycentric_coord = float3(1.0 - attrib.bary.x - attrib.bary.y, attrib.bary.x, attrib.bary.y);

    Mesh mesh = meshes[InstanceID()]; // corresponding to the mesh_index we passed in
    uint3 indices = load_indices_from_mesh(mesh);
    Vertices vertices = load_vertices_from_mesh(mesh, indices);

    float3 shading_normal = vertices.v0.normal * barycentric_coord.x +
        vertices.v1.normal * barycentric_coord.y +
        vertices.v2.normal * barycentric_coord.z;
    const float3 geometric_normal = normalize(cross(vertices.v1.position - vertices.v0.position, vertices.v2.position - vertices.v0.position));

    float4 color = load_color_from_mesh(mesh, indices, barycentric_coord);

    float2 uv0 = asfloat(draw_datas.Load2(indices.x * sizeof(float2) + mesh.uv_offset));
    float2 uv1 = asfloat(draw_datas.Load2(indices.y * sizeof(float2) + mesh.uv_offset));
    float2 uv2 = asfloat(draw_datas.Load2(indices.z * sizeof(float2) + mesh.uv_offset));
    const float2 uv = uv0 * barycentric_coord.x + uv1 * barycentric_coord.y + uv2 * barycentric_coord.z;

    uint material_id = draw_datas.Load(indices.x * sizeof(uint) + mesh.mat_id_offset);
    Material material = draw_datas.Load<Material>(material_id * sizeof(Material) + mesh.mat_data_offset);

    // 2. Calculate LOD based on ray cone
    // See Paper https://media.contentapi.ea.com/content/dam/ea/seed/presentations/2019-ray-tracing-gems-chapter-20-akenine-moller-et-al.pdf
    // Used equation 34. to calculate the final lod
    
    // calculate delta constants
    const float3 v0_pos_ws = mul(ObjectToWorld3x4(), float4(vertices.v0.position, 1.0));
    const float3 v1_pos_ws = mul(ObjectToWorld3x4(), float4(vertices.v1.position, 1.0));
    const float3 v2_pos_ws = mul(ObjectToWorld3x4(), float4(vertices.v2.position, 1.0));

    const float3 ray_hit_pos = WorldRayOrigin() + WorldRayDirection() * RayTCurrent();
    const float hit_distance = length(ray_hit_pos - WorldRayOrigin());

    const float delta_constants = 0.5 * log2(twice_uv_area(uv0, uv1, uv2) / twice_triangle_area(v0_pos_ws, v1_pos_ws, v2_pos_ws));
    const float curr_cone_width = payload.ray_cone.cone_width_at(hit_distance);
    const float3 geometric_normal_ws = normalize(mul(ObjectToWorld3x4(), float4(geometric_normal, 0.0)));

    // albedo map
    float4 albedo_texel = 1.0.xxxx;
    if ((mesh.texture_mask & TEXTURE_MASK_ALBEDO_BIT) != 0)
    {
        BindlessTextureWithLod albedo_tex = ray_cone_sample_texture_with_lod(
            material.albedo_map, delta_constants, curr_cone_width,
            geometric_normal_ws, WorldRayDirection()
        );
        albedo_texel = albedo_tex.sample_tex(sampler_llr, uv);
    }
    float3 albedo = albedo_texel.rgb * float4(material.base_color).rgb * color.rgb;
    
    // specular map
    float metalness = material.metalness;
    float roughness = clamp(perceptual_roughness_to_roughness(material.roughness), 1e-3, 1.0); // In reality, no object is purely smooth
    if ((mesh.texture_mask & TEXTURE_MASK_ALBEDO_BIT) != 0)
    {
        BindlessTextureWithLod specular_tex = ray_cone_sample_texture_with_lod(
            material.specular_map, delta_constants, curr_cone_width,
            geometric_normal_ws, WorldRayDirection()
        );
        float4 specular_texel = specular_tex.sample_tex(sampler_llr, uv);

        metalness *= specular_texel.z;
        float peceptual_roughness = material.roughness * specular_texel.y;
        roughness = clamp(perceptual_roughness_to_roughness(peceptual_roughness), 1e-3, 1.0); // In reality, no object is purely smooth
    }

    // normal map
    if ((mesh.texture_mask & TEXTURE_MASK_NORMAL_BIT) != 0)
    {
        float4 tangent0 = asfloat(draw_datas.Load4(indices.x * sizeof(float4) + mesh.tangent_offset));
        float4 tangent1 = asfloat(draw_datas.Load4(indices.y * sizeof(float4) + mesh.tangent_offset));
        float4 tangent2 = asfloat(draw_datas.Load4(indices.z * sizeof(float4) + mesh.tangent_offset));

        float3 bitangent0 = normalize(cross(vertices.v0.normal, tangent0.xyz) * tangent0.w);
        float3 bitangent1 = normalize(cross(vertices.v1.normal, tangent1.xyz) * tangent1.w);
        float3 bitangent2 = normalize(cross(vertices.v2.normal, tangent2.xyz) * tangent2.w);

        const float3 tangent = tangent0.xyz * barycentric_coord.x + 
            tangent1.xyz * barycentric_coord.y + 
            tangent2.xyz * barycentric_coord.z;
        const float3 bitangent = bitangent0 * barycentric_coord.x + 
            bitangent1* barycentric_coord.y + 
            bitangent2 * barycentric_coord.z;

        BindlessTextureWithLod normal_tex = ray_cone_sample_texture_with_lod(
            material.normal_map, delta_constants, curr_cone_width,
            geometric_normal_ws, WorldRayDirection()
        );
        float4 normal_texel = normal_tex.sample_tex(sampler_llce, uv);

        float3 normal_ts = float3(normal_texel.xy * 2.0 - 1.0, 0.0); // remap from [0, 1] to [-1, 1]
        normal_ts.z = sqrt(max(0.001, 1.0 - dot(normal_ts.xy, normal_ts.xy))); // normal in normal map is already normalized

        float3x3 tbn_matrix = float3x3(tangent, bitangent, shading_normal); // object sapce to tangent space matrix
        float3 normal_os = mul(normal_ts, tbn_matrix);

        shading_normal = normal_os;
    }

    GBuffer gbuffer = GBuffer::zero();
    gbuffer.albedo = albedo;
    gbuffer.normal = normalize(mul(ObjectToWorld3x4(), float4(shading_normal, 0.0)));
    gbuffer.metalness = metalness;
    gbuffer.roughness = roughness;

    // force double-sided
    if (dot(WorldRayDirection(), gbuffer.normal) > 0) {
        gbuffer.normal *= -1;
    }

    payload.packed_gbuffer = gbuffer.pack();
    payload.t = RayTCurrent();
}