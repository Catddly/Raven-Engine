#ifndef _IBL_LIGHTING_HLSL_
#define _IBL_LIGHTING_HLSL_

#include "../brdf.hlsl"
#include "../multi_scatter_compensate.hlsl"

struct Ibl 
{
    SpecularBrdf specular_brdf;

    static Ibl from_brdf(SpecularBrdf spec_brdf)
    {
        Ibl ibl;
        ibl.specular_brdf = spec_brdf;
        return ibl;
    }

    static float3 sample_irradiance(float3 normal)
    {
        return CONVOLVED_CUBEMAP.SampleLevel(sampler_llce, normal, 0).rgb;
    }

    static float3 sample_prefilter(float3 reflection, float roughness)
    {
        const float mip_level = roughness * (9.0 - 1.0); // TODO: replace to max LOD level
        return PREFILTERED_CUBEMAP.SampleLevel(sampler_llce, reflection, mip_level).rgb;
    }

    float3 eval_gbuffer(GBuffer gbuffer, float3 wo, float3 reflect, in MultiScatterCompensate compensate, float3 diff_reflectance)
    {
        const float3 normal = gbuffer.normal;
        const float roughness = gbuffer.roughness;

        float3 irradiance_sample = sample_irradiance(normal);
        float3 prefilter_sample = sample_prefilter(reflect, roughness);

        return compensate.compensate_ibl(irradiance_sample, prefilter_sample, diff_reflectance);
    }
};

#endif