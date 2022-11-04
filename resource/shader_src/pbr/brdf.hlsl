#ifndef _BRDF_HLSL_
#define _BRDF_HLSL_

#include "../math/constants.hlsl"
#include "../math/math.hlsl"
#include "../defer/gbuffer.hlsl"
#include "../common/roughness_adjust.hlsl"

#include "../color/luminance.hlsl"

#define USE_GGX_HEIGHT_CORRELATED_G_TERM 1

#define MIN_DIELECTRICS_REFLECTIVITY 0.04

struct BrdfResult
{
    // the evaluated value
    float3 value;
    float3 energy_preserve_ratio;
    float  pdf;
    float3 value_over_pdf;

    static BrdfResult invalid()
    {
        BrdfResult result;
        result.value = 0.0.xxx;
        result.energy_preserve_ratio = 0.0.xxx;
        result.pdf = 0.0;
        result.value_over_pdf = 0.0;
        return result;
    }
};

struct BrdfSample
{
    float3 wi;
    float3 weight;

    static BrdfSample invalid()
    {
        BrdfSample result;
        result.wi = float3(0.0, 0.0, -1.0);
        result.weight = 0.0.xxx;
        return result;
    }

    // perform sample on tangent space
    // is wi.z is negative, means this vector is on the opposite direction of the hemisphere
    bool is_valid()
    {
        return wi.z > 1e-7;
    }
};

// G in DFG
// Describes the self-shadowing property of the microfacets. 
// When a surface is relatively rough, the surface's microfacets can overshadow other microfacets reducing the light the surface reflects.
struct ShadowMaskTermSmith
{
    float g2;
    float g2_over_g1_wo;

    // split term for G term calculation.
    static float smith_ggx_split(float ndots, float alpha2)
    {
        const float numerator = 2 * ndots;
        const float term_sqrt = alpha2 + (1.0 - alpha2) * (ndots * ndots);
        const float denom = sqrt(term_sqrt) + ndots;
        return numerator / denom;
    }

    // Smith G1 term (masking function) further optimized for GGX distribution (by substituting G_a into G1_GGX)
    static float smith_ggx_split_Ga(float alpha2, float ndots2) {
        return 2.0f / (sqrt(((alpha2 * (1.0f - ndots2)) + ndots2) / ndots2) + 1.0f);
    }

    static float smith_ggx_height_correlated(float ndotv, float ndotl, float alpha2)
    {
        const float lambda_1 = ndotv * sqrt(alpha2 + ndotl * (ndotl - alpha2 * ndotl));
        const float lambda_2 = ndotl * sqrt(alpha2 + ndotv * (ndotv - alpha2 * ndotv));
        return 0.5 / (lambda_1 + lambda_2);
    }

    static float smith_ggx_height_correlated_over_g1(float ndotv, float ndotl, float alpha2)
    {
        float g1_v = smith_ggx_split_Ga(alpha2, ndotv * ndotv);
        float g1_l = smith_ggx_split_Ga(alpha2, ndotl * ndotl);
        return g1_l / (g1_v + g1_l - g1_v * g1_l);
    }

    static ShadowMaskTermSmith eval(float ndotv, float ndotl, float alpha2) {
        ShadowMaskTermSmith result;
    #if USE_GGX_HEIGHT_CORRELATED_G_TERM
        result.g2 = smith_ggx_height_correlated(ndotv, ndotl, alpha2);
        result.g2_over_g1_wo = smith_ggx_height_correlated_over_g1(ndotv, ndotl, alpha2);
    #else
        result.g2 = smith_ggx_split(ndotl, a2) * smith_ggx_split(ndotv, a2);
        result.g2_over_g1_wo = smith_ggx_split(ndotl, a2);
    #endif
        return result;
    }
};

// See: "An efficient and Physically Plausible Real-Time Shading Model" in ShaderX7 by Schuler
// Attenuates F90 for very low F0 values
float3 get_F90(float3 F0)
{
    const float t = (1.0f / MIN_DIELECTRICS_REFLECTIVITY);
	return min(1.0f, t * rgb_color_to_luminance(F0));
}

// F in DFG
// The Fresnel equation describes the ratio of surface reflection at different surface angles.
// This equation implicitly contain ks term.
// cos_theta is the dot product of half vector and view vector, F0 is the lerped value by metallic (a constants, albedo)
float3 fresnel_schlick(float3 F0, float3 F90, float ndots)
{
    return F0 + (F90 - F0) * pow(1.0f - ndots, 5.0f);
}

struct SpecularBrdf
{
    // this albedo is adjusted (i.e. only have meaning in specular part)
    float3 F0;
    // TODO: improve this (add alpha and alpha2 here)
    float  roughness;

    static SpecularBrdf zero()
    {
        SpecularBrdf spec;
        spec.F0 = 0.0.xxx;
        spec.roughness = 0.0;
        return spec;
    }

    // D in DFG
    // Normal distribution function Trowbridge-Reitz GGX
    // Approximates the amount the surface's microfacets are aligned to the halfway vector, 
    // influenced by the roughness of the surface
    // This is the primary function approximating the microfacets.
    static float ndf_ggx(float alpha2, float ndoth) // h is half vector of (light, view)
    {
        const float denom = (ndoth * ndoth * (alpha2 - 1.0)) + 1.0;
        return alpha2 / (PI * denom * denom);
    }

    // Sample a microfacet normal for the GGX normal distribution using VNDF method. (visible NDF)
    // See http://jcgt.org/published/0007/04/01/
    static float3 sample_ggx_vndf(float3 wo, float alpha, float2 urand)
    {
        // isotropic
        float alpha_x = alpha;
        float alpha_y = alpha;

        // Section 3.2: transforming the view direction to the hemisphere configuration
        float3 Vh = normalize(float3(alpha_x * wo.x, alpha_y * wo.y, wo.z));

        // Section 4.1: orthonormal basis (with special case if cross product is zero)
        float lensq = Vh.x * Vh.x + Vh.y * Vh.y;
        float3 T1 = lensq > 0.0 ? float3(-Vh.y, Vh.x, 0.0) * rsqrt(lensq) : float3(1.0, 0.0, 0.0);
        float3 T2 = cross(Vh, T1);

        // Section 4.2: parameterization of the projected area
        float r = sqrt(urand.x);
        float phi = 2.0 * PI * urand.y;
        float t1 = r * cos(phi);
        float t2 = r * sin(phi);
        float s = 0.5 * (1.0 + Vh.z);
        t2 = (1.0 - s) * sqrt(1.0 - t1 * t1) + s * t2;

        // Section 4.3: reprojection onto hemisphere
        float3 Nh = t1 * T1 + t2 * T2 + sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)) * Vh;

        // Section 3.4: transforming the normal back to the ellipsoid configuration
        float3 Ne = normalize(float3(alpha_x * Nh.x, alpha_y * Nh.y, max(0.0, Nh.z)));
        return Ne;
    }

    // PDF of sampling a reflection vector L using 'sample'.
    // Note that PDF of sampling given microfacet normal is (G1 * D) when vectors are in local space (in the hemisphere around shading normal). 
    // Remaining terms (1.0f / (4.0f * NdotV)) are specific for reflection case, and come from multiplying PDF by jacobian of reflection operator
    float pdf(float ndoth, float ndotv, float alpha2)
    {
        ndoth = max(0.00001, ndoth);
	    ndotv = max(0.00001, ndotv);
	    return (ndf_ggx(max(0.00001, alpha2), ndoth) * ShadowMaskTermSmith::smith_ggx_split_Ga(alpha2, ndotv * ndotv)) / (4.0 * ndotv);
    }

    BrdfSample sample(float3 wo, float2 urand)
    {
        const float alpha = perceptual_roughness_to_roughness(roughness); 

        // sample a microfacet normal (H) in local space (Gm)
        float3 h;
        if (alpha == 0.0f) {
            // fast path for zero roughness (perfect reflection), also prevents NaNs appearing due to divisions by zeroes
            h = float3(0.0f, 0.0f, 1.0f);
        } else {
            // for non-zero roughness, this calls VNDF sampling for GG-X distribution or Walter's sampling for Beckmann distribution
            h = sample_ggx_vndf(wo, alpha, urand);
        }

        // Reflect view direction to obtain light vector
	    const float3 wi = reflect(-wo, h);

        // invalid sample direction
        if (h.z <= 1e-6 || wi.z <= 1e-6 || wo.z <= 1e-6) {
			return BrdfSample::invalid();
		}

        BrdfSample result;
        // note: hdotl is same as hdotl here
        // clamp dot products here to small value to prevent numerical instability. Assume that rays incident from below the hemisphere have been filtered
        float hdotl = max(0.00001, min(1.0, dot(h, wi)));
        // const float3 normal = float3(0.0f, 0.0f, 1.0f);
        // float ndotl = max(0.00001, min(1.0, dot(normal, wi)));
        // float ndotv = max(0.00001, min(1.0, dot(normal, wo)));

        float3 f = fresnel_schlick(F0, get_F90(F0), hdotl);

        // Calculate weight of the sample specific for selected sampling method
        // (this is microfacet BRDF divided by PDF of sampling method - notice how most terms cancel out)
        result.weight = f * ShadowMaskTermSmith::smith_ggx_height_correlated_over_g1(wo.z, wi.z, alpha * alpha);
        result.wi = wi;
        return result;
    }

    BrdfResult eval_ndotl_weighted(float3 wi, float3 wo)
    {
        if (wi.z < 0.0 || wo.z < 0.0)
        {
            return BrdfResult::invalid();
        }

        BrdfResult result;

        // you can call it m or h whatever. They use m in shadowing-masking for half vector.
        const float3 m = normalize(wi + wo);
        const float ndoth = m.z;
        const float alpha = perceptual_roughness_to_roughness(roughness);
        const float alpha2 = alpha * alpha;

        // ks is in the f term
        ShadowMaskTermSmith smith = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha2);

        const float  d = ndf_ggx(max(0.00001, alpha2), ndoth);
        const float  g = smith.g2;
        const float3 f = fresnel_schlick(F0, get_F90(F0), saturate(dot(wi, m)));

        result.value_over_pdf = f * smith.g2_over_g1_wo;
        result.value = f * (d * g * wi.z);
        result.energy_preserve_ratio = 1.0.xxx - f;
        result.pdf = (d * ShadowMaskTermSmith::smith_ggx_split_Ga(alpha2, wo.z * wo.z)) / (4.0 * wo.z);
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

float3 fresnel_schlick_roughness(float ndotv, float3 F0, float roughness)
{
    return F0 + (max((1.0 - roughness).xxx, F0) - F0) * pow(clamp(1.0 - ndotv, 0.0, 1.0), 5.0);
}

struct DiffuseBrdf
{
    float3 reflectance;

    static DiffuseBrdf zero()
    {
        DiffuseBrdf result;
        result.reflectance = 0.0.xxx;
        return result;
    }

    // return a cosine-term wrighted pdf
    float pdf(float3 wi)
    {
        if (wi.z < 0.0)
        {
            return 0.0;
        }

        return PI_RECIP_ONE * wi.z;
    }

    BrdfSample sample(float3 wo, float3 wi, float2 urand)
    {
        BrdfSample result;
        // result.pdf = PI_RECIP_ONE * wi.z; // cosine-weighted pdf
        // result.value_over_pdf = reflectance;
        // result.value = result.value_over_pdf * result.pdf;
        // result.energy_preserve_ratio = 0.0.xxx; // no meaning, ks is already in fresnel term.

		// sample diffuse ray using cosine-weighted hemisphere sampling 
        float3 sample_dir = sample_hemisphere(urand);

		// function 'diffuseTerm' is predivided by PDF of sampling the cosine weighted hemisphere
        // this value is BrdfResult.value_over_pdf
        float3 sample_weight = reflectance;

#if 0
		// sample a half-vector of specular BRDF. Note that we're reusing random variable 'u' here, but correctly it should be an new independent random number
        NdfSamples ndf_sample = SpecularBrdf::sample_ggx_vndf(wo, alpha, urand);

        // clamp HdotL to small value to prevent numerical instability. Assume that rays incident from below the hemisphere have been filtered
		float vdoth = max(0.00001, min(1.0, dot(wo, ndf_sample.normal)));
		sample_weight *= (1.0.xxx - evalFresnel(data.specularF0, shadowedF90(data.specularF0), VdotH));
#endif

        result.wi = sample_dir;
        result.weight = sample_weight;
        return result;
    }

    BrdfResult eval_ndotl_weighted(float3 wi)
    {
        if (wi.z <= 0.0)
        {
            return BrdfResult::invalid();
        }

        BrdfResult result;
        result.pdf = PI_RECIP_ONE * wi.z; // cosine-weighted pdf
        result.value_over_pdf = reflectance;
        result.value = result.value_over_pdf * result.pdf;
        result.energy_preserve_ratio = 0.0.xxx; // no meaning, ks is already in fresnel term.
        return result;
    }
};

struct Brdf 
{
    DiffuseBrdf  diffuse_brdf;
    SpecularBrdf specular_brdf;

    static DiffuseBrdf metalness_to_diffuse_reflectance(float3 albedo, float metelness)
    {
        DiffuseBrdf diff = DiffuseBrdf::zero();
        diff.reflectance = albedo * (1.0 - metelness);
        return diff;
    }

    static SpecularBrdf metalness_to_specular_F0(float3 albedo, float metelness)
    {
        SpecularBrdf spec = SpecularBrdf::zero();
        spec.F0 = lerp(MIN_DIELECTRICS_REFLECTIVITY.xxx, albedo, metelness);
        return spec;
    }

    // static void metalness_brdf_interpolation(inout DiffuseBrdf diff, inout SpecularBrdf spec, float metalness)
    // {
    //     const float3 gbuffer_albedo = diff.spe;

    //     // lerp from 0.04 to albedo using metalness
    //     // It is the F0 in https://learnopengl.com/PBR/Lighting
    //     float3 spec_lerped_albedo = lerp(spec.albedo, gbuffer_albedo, metalness);
    //     // remember that ks in the F term in brdf.
    //     // kd + ks = 1. so kd = 1.0 - ks.
    //     // so kd = (1.0.xxx - F) * (1.0 - metalness)
    //     // Why multiply one minus metalness here, because when a material is fully dielectric,
    //     // it diffuse color become completely absorb by the electronics.
    //     // (i.e. metallic surfaces don't refract light and thus have no diffuse reflections)
    //     float3 diff_lerped_albedo = gbuffer_albedo * max(0.0, 1.0 - metalness);

    //     float3 albedo_boost = metalness_albedo_boost(metalness, gbuffer_albedo);

    //     diff.albedo = min(1.0, diff_lerped_albedo * albedo_boost);
    //     spec.albedo = min(1.0, spec_lerped_albedo * albedo_boost);
    // }

    static Brdf from_gbuffer(GBuffer gbuffer)
    {
        Brdf result;

        DiffuseBrdf  diffuse  = metalness_to_diffuse_reflectance(gbuffer.albedo, gbuffer.metalness);
        SpecularBrdf specular = metalness_to_specular_F0(gbuffer.albedo, gbuffer.metalness);
        specular.roughness = gbuffer.roughness;

        result.diffuse_brdf = diffuse;
        result.specular_brdf = specular;
        return result;
    }

    float3 eval_ndotl_weighted(float3 wi, float3 wo)
    {
        if (wi.z <= 0 || wo.z <= 0)
        {
            return 0.0.xxx;
        }

        BrdfResult diff = diffuse_brdf.eval_ndotl_weighted(wi);
        BrdfResult spec = specular_brdf.eval_ndotl_weighted(wi, wo);

        // TODO: Conservation Of Energy
        // The higher the roughness is, the more energy lost.
        // Due to G term in brdf (the shadowing-masking term) do not consider multi-bouncing of the microsurface.
        // We lost energy when the microsurface is more uneven.

        return diff.value * spec.energy_preserve_ratio + spec.value;
    }
};

#endif