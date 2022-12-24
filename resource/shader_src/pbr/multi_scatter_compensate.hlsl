#ifndef _MULTI_SCATTER_COMPENSATE_HLSL_
#define _MULTI_SCATTER_COMPENSATE_HLSL_

#include "../common/bindless_resources.hlsl"
#include "../common/immutable_sampler.hlsl"

#include "../pbr/brdf_result.hlsl"

// Fresnel term but take roughness into account
float3 fresnel_schlick_roughness(float ndotv, float3 F0, float roughness)
{
    return F0 + (max((1.0 - roughness).xxx, F0) - F0) * pow((1.0 - ndotv), 5.0);
}

// Used to compensate the energy lost because of the lack of the multi-scatter of the brdf microfacet model.
// The higher the roughness is, the more energy lost.
// Due to G term in brdf (the shadowing-masking term) do not consider multi-bouncing of the microsurface.
// We lost energy when the microsurface is more uneven.
struct MultiScatterCompensate
{
    // Also be written in the paper as FssEss
    float3 single_scatter;
    // It is the energy of the single scatter, in order to compensate the lost energy,
    // multiply it by a multiplier.
    float3 preintegrated_reflection;
    float3 preintegrated_reflection_multiplier;
    float3 preintegrated_transmission_fraction;
    float2 env_brdf;
    float3 F0;

    static float2 sample_env_brdf(float3 wo, float roughness)
    {
        const float2 env_brdf_uv = float2(wo.z, roughness);
        return bindless_textures[BRDF_LUT_BINDLESS_INDEX].SampleLevel(sampler_lnce, env_brdf_uv, 0.0).xy;
    }

    static MultiScatterCompensate compensate_for(float3 wo, float roughness, float3 F0)
    {
        float3 ks = fresnel_schlick_roughness(wo.z, F0, roughness);
        float2 env_brdf = sample_env_brdf(wo, roughness);

        // See https://blog.selfshadow.com/publications/turquin/ms_comp_final.pdf
        float  e_ss = env_brdf.x + env_brdf.y;
        float3 f_ss = lerp(F0, 1.0, pow(max(0.0, 1.0 - wo.z), 5));
        float3 multiplier = 1.0 + f_ss * (1.0 - e_ss) / e_ss;

        MultiScatterCompensate compensate;
        compensate.single_scatter = ks * env_brdf.x + env_brdf.y;
        compensate.preintegrated_reflection = compensate.single_scatter * multiplier;
        compensate.preintegrated_reflection_multiplier = multiplier;
        // this is just the remaining part of the refleced energy
        compensate.preintegrated_transmission_fraction = 1.0 - compensate.preintegrated_reflection;
        compensate.env_brdf = env_brdf;
        compensate.F0 = F0;
        return compensate;
    }

    float3 compensate_ibl(in float3 irradiance, in float3 radiance, float3 diffuse_reflectance)
    {
        // multiple scattering compensate, from Fdez-Aguera.
        // See https://www.jcgt.org/published/0008/01/03/paper.pdf
        float Ems = (1.0 - (env_brdf.x + env_brdf.y));
        float3 F_avg = F0 + (1.0 - F0) / 21.0;
        float3 FmsEms = Ems * single_scatter * F_avg / (1.0 - F_avg * Ems);

        float3 k_D = diffuse_reflectance * (1.0 - single_scatter - FmsEms);

        return single_scatter * radiance + (FmsEms + k_D) * irradiance;
    }

    float3 compensate_brdf(in BrdfResult diffuse_brdf, in BrdfResult specular_brdf, float3 wo)
    {
        return diffuse_brdf.value * specular_brdf.transmission_ratio + specular_brdf.value * preintegrated_reflection_multiplier;
    }

    float3 compensate_brdf_directional_light(in BrdfResult diffuse_brdf, in BrdfResult specular_brdf, float3 wi, float3 wo)
    {
        // from kajiya.

        // TODO: multi-scattering on the interface can bend secondary lobes away from
        // the evaluated direction, which is particularly apparent for directional lights.
        // In the latter case, the following term works better.
        // On the other hand, this will result in energy loss for non-directional lights
        // since the lobes are just redirected, and not lost.
        const float3 mult_directional = lerp(1.0, preintegrated_reflection_multiplier, sqrt(abs(wi.z)));

        return diffuse_brdf.value * specular_brdf.transmission_ratio + specular_brdf.value * mult_directional;
    }
};

#endif