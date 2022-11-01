#ifndef _BINDLESS_RESOURCES_HLSL_
#define _BINDLESS_RESOURCES_HLSL_

#include "mesh.hlsl"

[[vk::binding(0, 1)]] ByteAddressBuffer        draw_datas;
[[vk::binding(1, 1)]] StructuredBuffer<Mesh>   meshes;
[[vk::binding(2, 1)]] StructuredBuffer<float4> bindless_texture_sizes;
[[vk::binding(3, 1)]] Texture2D                bindless_textures[];

#endif