#ifndef _MESH_HLSL_
#define _MESH_HLSL_

#include "pack_unpack.hlsl"

// Same in raven-render::renderer::mesh_renderer GpuMesh (line: 27)
struct Mesh {
    // Used to index the universal draw data buffer
    uint vertex_offset;
    uint color_offset;
    uint uv_offset;
    uint tangent_offset;
    uint index_offset;
    uint mat_id_offset;
    // Used to index the universal material data buffer
    uint mat_data_offset;

    uint texture_mask;
};

static const uint TEXTURE_MASK_ALBEDO_BIT   = (1 << 0);
static const uint TEXTURE_MASK_NORMAL_BIT   = (1 << 1);
static const uint TEXTURE_MASK_SPECULAR_BIT = (1 << 2);
static const uint TEXTURE_MASK_EMISSIVE_BIT = (1 << 3);

struct PackedVertex;

struct Vertex {
    float3 position;
    float3 normal;

    PackedVertex pack();
};

struct PackedVertex {
    float4 data;

    Vertex unpack();
};

PackedVertex Vertex::pack() {
    PackedVertex pack;
    pack.data.xyz = position;
    pack.data.w = pack_normal_11_10_11(normal);
    return pack;
}

Vertex PackedVertex::unpack() {
    Vertex vertex;
    vertex.position = data.xyz;
    vertex.normal = unpack_normal_11_10_11(data.w);
    return vertex;
}

#endif