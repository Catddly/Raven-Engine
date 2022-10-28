#ifndef _BRDF_HLSL_
#define _BRDF_HLSL_

#include "../math/constants.hlsl"
#include "../defer/gbuffer.hlsl"

#define USE_GGX_CORRELATED_G_TERM 1

// D in DFG
// Approximates the amount the surface's microfacets are aligned to the halfway vector, 
// influenced by the roughness of the surface
// This is the primary function approximating the microfacets.
// float normal_distribution_GGX(float3 normal, float3 half_vec, float roughness) 
// {
//     float a        = roughness * roughness;
//     float a2       = a * a;
//     float n_dot_h  = max(dot(normal, half_vec), 0.0);
//     float n_dot_h2 = n_dot_h * n_dot_h;
	
//     float denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
//     denom = PI * denom * denom;
	
//     return a2 / denom;
// }

// float geometry_schlick_GGX(float n_dot_v, float k) 
// {
//     float nom   = n_dot_v;
//     float denom = n_dot_v * (1.0 - k) + k;
	
//     return nom / denom;
// }

// Map roughness when calculating geometry function in direct lighting.
// float map_k_direct_lighting(float roughness) 
// {
//     return pow(roughness + 1, 2.0) / 8.0;
// }

// Map roughness when calculating geometry function in IBL(Image based lighting).
// float map_k_IBL(float roughness) 
// {
//     return roughness * roughness / 2.0;
// }

// G in DFG
// Describes the self-shadowing property of the microfacets. 
// When a surface is relatively rough, the surface's microfacets can overshadow other microfacets reducing the light the surface reflects.
// Use Smith's Method to split calculation into multiplication.
// Here k is a remapping of roughness value. See map_k_direct_lighting() and map_k_IBL().
// float3 geomotry_smith(float3 normal, float3 view, float3 light, float k) 
// {
//     float n_dot_v = max(dot(normal, view), 0.0);
//     float n_dot_l = max(dot(normal, light), 0.0);
//     float ggx_0 = geometry_schlick_GGX(n_dot_v, k);
//     float ggx_1 = geometry_schlick_GGX(n_dot_l, k);
	
//     return ggx_0 * ggx_1;
// }

struct BrdfResult
{
    float3 value;
    float3 energy_preserve_ratio;

    static BrdfResult invalid()
    {
        BrdfResult result;
        result.value = 0.0.xxx;
        result.energy_preserve_ratio = 0.0.xxx;
        return result;
    }
};

struct DiffuseBrdf
{
    float3 albedo;

    BrdfResult eval(float3 wi)
    {
        BrdfResult result;
        // ndotv must be positive
        result.value = wi.z > 0.0 ? (PI_RECIP_ONE * albedo) : 0.0.xxx;
        result.energy_preserve_ratio = 0.0.xxx; // no meaning, ks is already in fresnel term.
        return result;
    }
};

float3 fresnel_schlick(float3 F0, float3 F90, float cos_theta /* ldoth */)
{
    return lerp(F0, F90, pow(max(0.0, 1.0 - cos_theta), 5));
}

// F in DFG
// The Fresnel equation describes the ratio of surface reflection at different surface angles.
// This equation implicitly contain ks term.
// cos_theta is the dot product of half vector and view vector, F0 is the lerped value by metallic (a constants, albedo)
// float3 fresnel_schlick(float cos_theta, float3 F0)
// {
//     return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0); 
// }

struct ShadowMaskTermSmith
{
    float g;

    // split term for G term calculation.
    static float smith_ggx_split(float ndotv, float r2)
    {
        const float numerator = 2 * ndotv;
        const float term_sqrt = r2 + (1.0 - r2) * (ndotv * ndotv);
        const float denom = sqrt(term_sqrt) + ndotv;
        return numerator / denom;
    }

    static float smith_ggx_correlated(float ndotv, float ndotl, float a2)
    {
        const float numerator = 2 * ndotv * ndotl;
        const float lambda_1 = ndotv * sqrt(a2 + (1.0 - a2) * (ndotl * ndotl));
        const float lambda_2 = ndotl * sqrt(a2 + (1.0 - a2) * (ndotv * ndotv));
        return numerator / (lambda_1 + lambda_2);
    }

    static ShadowMaskTermSmith eval(float ndotv, float ndotl, float a2) {
        ShadowMaskTermSmith result;
    #if USE_GGX_CORRELATED_G_TERM
        result.g = smith_ggx_correlated(ndotv, ndotl, a2);
    #else
        result.g = smith_ggx_split(ndotl, a2) * smith_ggx_split(ndotv, a2);
    #endif
        return result;
    }
};

struct SpecularBrdf
{
    // this albedo is adjusted (i.e. only have meaning in specular part)
    float3 albedo;
    float  roughness;

    // Normal distribution function Trowbridge-Reitz GGX
    static float ndf_ggx(float roughness2, float cos_theta /* ndoth */) // h is half vector of (light, view)
    {
        const float denom = (cos_theta * cos_theta) * (roughness2 - 1.0) + 1.0;
        return roughness2 / (PI * denom * denom);
    }

    BrdfResult eval(float3 wi, float3 wo)
    {
        if (wi.z < 0.0 || wo.z < 0.0)
        {
            return BrdfResult::invalid();
        }

        BrdfResult result;

        const float r2 = roughness * roughness;
        // you can call it m or h whatever. They use m in shadowing-masking for half vector.
        const float3 m = normalize(wi + wo);
        // this is ndoth
        const float cos_theta = m.z;

        // ks is in the f term
        ShadowMaskTermSmith smith = ShadowMaskTermSmith::eval(wo.z, wi.z, r2);

        const float3 f = fresnel_schlick(albedo, 1.0.xxx, dot(m, wi));
        const float  n = ndf_ggx(r2, cos_theta);
        const float  g = smith.g;

        // ndotv must be positive
        result.value = n * f * g / (4.0 * wi.z * wo.z);
        result.energy_preserve_ratio = 1.0.xxx - f;
        return result;
    }
};

// From kajiya, In shader layered_brdf.hlsl, line 11.
// Metalness other than 0.0 and 1.0 loses energy due to the way diffuse albedo
// is spread between the specular and diffuse layers. Scaling both the specular
// and diffuse albedo by a constant can recover this energy.
// This is a reasonably accurate fit (RMSE: 0.0007691) to the scaling function.
float3 metalness_albedo_boost(float metalness, float3 diffuse_albedo) {
    static const float a0 = 1.749;
    static const float a1 = -1.61;
    static const float e1 = 0.5555;
    static const float e3 = 0.8244;

    const float x = metalness;
    const float3 y = diffuse_albedo;
    const float3 y3 = y * y * y;

    return 1.0 + (0.25-(x-0.5)*(x-0.5)) * (a0+a1*abs(x-0.5)) * (e1*y + e3*y3);
}

struct Brdf 
{
    DiffuseBrdf  diffuse_brdf;
    SpecularBrdf specular_brdf;

    static void lerp_albedo_by_metallic(inout DiffuseBrdf diff, inout SpecularBrdf spec, float metallic)
    {
        const float3 gbuffer_albedo = diff.albedo;

        // lerp from 0.04 to albedo using metallic
        // It is the F0 in https://learnopengl.com/PBR/Lighting
        float3 spec_lerped_albedo = lerp(spec.albedo, gbuffer_albedo, metallic);
        // remember that ks in the F term in brdf.
        // kd + ks = 1. so kd = 1.0 - ks.
        // so kd = (1.0.xxx - F) * (1.0 - metallic)
        // Why multiply one minus metallic here, because when a material is fully dielectric,
        // it diffuse color become completely absorb by the electronics.
        // (i.e. metallic surfaces don't refract light and thus have no diffuse reflections)
        float3 diff_lerped_albedo = gbuffer_albedo * max(0.0, 1.0 - metallic);

        float3 albedo_boost = metalness_albedo_boost(metallic, gbuffer_albedo);

        diff.albedo = min(1.0, diff_lerped_albedo * albedo_boost);
        spec.albedo = min(1.0, spec_lerped_albedo * albedo_boost);

        //diff.albedo = min(1.0, diff_lerped_albedo);
        //spec.albedo = min(1.0, spec_lerped_albedo);
    }

    static Brdf from_gbuffer_ndotv(GBuffer gbuffer)
    {
        Brdf result;

        DiffuseBrdf diffuse;
        diffuse.albedo = gbuffer.albedo;

        SpecularBrdf specular;
        specular.albedo = 0.04.xxx; // albedo is 0.04 when it is a dielectric
        specular.roughness = gbuffer.roughness;

        lerp_albedo_by_metallic(diffuse, specular, gbuffer.metallic);

        result.diffuse_brdf = diffuse;
        result.specular_brdf = specular;
        return result;
    }

    float3 eval(float3 wi, float3 wo)
    {
        if (wi.z <= 0 || wo.z <= 0) 
        {
            return 0.0.xxx;
        }

        const BrdfResult diff = diffuse_brdf.eval(wi);
        const BrdfResult spec = specular_brdf.eval(wi, wo);

        // TODO: Conservation Of Energy
        // The higher the roughness is, the more energy lost.
        // Due to G term in brdf (the shadowing-masking term) do not consider multi-bouncing of the microsurface.
        // We lost energy when the microsurface is more uneven.

        return diff.value * spec.energy_preserve_ratio + spec.value;
    }
};

#endif