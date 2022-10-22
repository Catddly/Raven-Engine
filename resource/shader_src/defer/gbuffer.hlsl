#ifndef _GBUFFER_HLSL_
#define _GBUFFER_HLSL_

#include "../common/pack_unpack.hlsl"
#include "../common/roughness_adjust.hlsl"

struct PackedGBuffer;

struct GBuffer {
    float3 albedo;
    float3 normal;
    float  metallic;
    float  roughness;

    static GBuffer zero() {
        GBuffer res;
        res.albedo = 0;
        res.normal = 0;
        res.metallic = 0;
        res.roughness = 0;
        return res;
    }

    PackedGBuffer pack();
};

struct PackedGBuffer {
    uint4 data;

    GBuffer unpack();

    static PackedGBuffer from_uint4(uint4 tex) {
        PackedGBuffer packed;
        packed.data = tex;
        return packed;
    }
};

PackedGBuffer GBuffer::pack() {
    float4 res = 0.0.xxxx;
    res.x = asfloat(pack_color_888_uint(albedo));
    res.y = pack_normal_11_10_11(normal);

    float2 mr = float2(metallic, roughness_to_perceptual_roughness(roughness));
    res.z = asfloat(pack_2x16f_uint(mr));
    // reserved
    res.w = 0;

    PackedGBuffer packed;
    packed.data = asuint(res);
    return packed;
}

GBuffer PackedGBuffer::unpack() {
    GBuffer gbuffer = GBuffer::zero();
    gbuffer.albedo = unpack_color_888_uint(data.x);
    gbuffer.normal = unpack_normal_11_10_11(asfloat(data.y));

    float2 mr = unpack_2x16f_uint(data.z);
    gbuffer.metallic = mr.x;
    gbuffer.roughness = perceptual_roughness_to_roughness(mr.y);

    return gbuffer;
}

#endif