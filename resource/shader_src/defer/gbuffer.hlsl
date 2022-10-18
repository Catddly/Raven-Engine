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
    float4 data;

    GBuffer unpack();
};

PackedGBuffer GBuffer::pack() {
    PackedGBuffer packed;

    uint4 res = 0;
    res.r = asfloat(pack_color_888_uint(albedo));
    res.g = pack_normal_11_10_11(normal);

    float2 mr = float2(metallic, roughness_to_perceptual_roughness(roughness));
    res.b = asfloat(pack_2x16f_uint(mr));
    // reserved
    res.a = 0;

    packed.data = asfloat(res);
    return packed;
}

GBuffer PackedGBuffer::unpack() {
    uint4 packed = asuint(data); 

    GBuffer gbuffer = GBuffer::zero();
    gbuffer.albedo = unpack_color_888_uint(packed.r);
    gbuffer.normal = unpack_normal_11_10_11(asfloat(packed.g));

    float2 mr = unpack_2x16f_uint(packed.b);

    gbuffer.metallic = mr.x;
    gbuffer.roughness = perceptual_roughness_to_roughness(mr.y);

    return gbuffer;
}

#endif