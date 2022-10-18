#ifndef _BINDLESS_HLSL_
#define _BINDLESS_HLSL_

#include "mesh.hlsl"

[[vk::binding(0, 1)]] ByteAddressBuffer      draw_datas;
[[vk::binding(1, 1)]] StructuredBuffer<Mesh> meshes;

#endif