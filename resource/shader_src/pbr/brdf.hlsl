#ifndef _BRDF_HLSL_
#define _BRDF_HLSL_

#include "../math/constants.hlsl"
#include "../math/math.hlsl"
#include "../math/coordinate.hlsl"
#include "../defer/gbuffer.hlsl"
#include "../common/roughness_adjust.hlsl"

#include "../color/luminance.hlsl"

#include "../common/immutable_sampler.hlsl"

#include "../pbr/multi_scatter_compensate.hlsl"
#include "brdf_result.hlsl"

#define USE_GGX_HEIGHT_CORRELATED_G_TERM 1

#define MIN_DIELECTRICS_REFLECTIVITY 0.04

#define BRDF_FORCE_DIFFUSE_SAMPLE_ONLY 0
#define BRDF_FORCE_SPECULAR_SAMPLE_ONLY 0

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
    
    static float smith_ggx_height_correlated(float ndotv, float ndotl, float alpha2)
    {
        const float lambda_1 = ndotv * sqrt(alpha2 + ndotl * (ndotl - alpha2 * ndotl));
        const float lambda_2 = ndotl * sqrt(alpha2 + ndotv * (ndotv - alpha2 * ndotv));
        return 2.0 * ndotl * ndotv / (lambda_1 + lambda_2);
    }

    static ShadowMaskTermSmith eval(float ndotv, float ndotl, float alpha2) {
        ShadowMaskTermSmith result;
    #if USE_GGX_HEIGHT_CORRELATED_G_TERM
        result.g2 = smith_ggx_height_correlated(ndotv, ndotl, alpha2);
        result.g2_over_g1_wo = result.g2 / smith_ggx_split(ndotv, alpha2);
    #else
        result.g2 = smith_ggx_split(ndotl, alpha2) * smith_ggx_split(ndotv, alpha2);
        result.g2_over_g1_wo = smith_ggx_split(ndotl, alpha2);
    #endif
        return result;
    }
};

// F in DFG
// The Fresnel equation describes the ratio of surface reflection at different surface angles.
// This equation implicitly contain ks term.
// cos_theta is the dot product of half vector and view vector, F0 is the lerped value by metallic (a constants, albedo)
float3 eval_fresnel_schlick(float3 f0, float3 f90, float ndots)
{
    return lerp(f0, f90, pow(max(0.0, 1.0 - ndots), 5.0));
}

struct SpecularBrdf
{
    // this albedo is adjusted (i.e. only have meaning in specular part)
    float3 F0;
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

    static float pdf_ggx_vn(float3 wo, float3 half_vec, float alpha2)
    {
        const float ndotv = wo.z;
        float g1 = ShadowMaskTermSmith::smith_ggx_split(wo.z, alpha2);
        float d = ndf_ggx(alpha2, half_vec.z);
        return g1 * d * max(0.0, dot(wo, half_vec)) / wo.z;
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
    float pdf(float3 wi, float3 wo, float3 m)
    {
        const float alpha2 = roughness * roughness;

        const float jacobian = 1.0 / (4.0 * dot(wi, m));
        const float pdf_half_vec = pdf_ggx_vn(wo, m, alpha2);

	    return pdf_half_vec * jacobian / wi.z;
    }

    BrdfSample sample(float3 wo, float2 urand)
    {
        const float alpha = roughness;
        const float alpha2 = alpha * alpha;

        // sample a microfacet normal (H) in local space (Gm)
        float3 h = sample_ggx_vndf(wo, alpha, urand);

        // reflect view direction to obtain light vector
	    const float3 wi = reflect(-wo, h);

        // invalid sample direction
        if (h.z <= 1e-6 || wi.z <= 1e-6 || wo.z <= 1e-6) {
			return BrdfSample::invalid();
		}

        BrdfSample result;
        const float d = ndf_ggx(alpha2, h.z);
        // note: hdotl is the same as hdotv here
        // clamp dot products here to small value to prevent numerical instability.
        // Assume that rays incident from below the hemisphere have been filtered.
        float3 f = eval_fresnel_schlick(F0, 1.0, max(0.00001, min(1.0, dot(h, wi))));

        // Calculate weight of the sample specific for selected sampling method
        // (this is microfacet BRDF divided by PDF of sampling method - notice how most terms cancel out)
        ShadowMaskTermSmith g_term = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha2);
        const float g = g_term.g2;

        result.pdf = pdf(wi, wo, h);
        result.value_over_pdf = f * g_term.g2_over_g1_wo;
        result.value = d * f * g / (4.0 * wi.z *wo.z);
        result.transmission_ratio = 1.0.xxx - f;
        //result.weight = f * g_term.g2_over_g1_wo;
        result.wi = wi;
        return result;
    }

    BrdfResult eval(float3 wi, float3 wo)
    {
        if (wi.z < 0.0 || wo.z < 0.0)
        {
            return BrdfResult::invalid();
        }

        BrdfResult result;

        // you can call it m or h whatever. They use m in shadowing-masking for half vector.
        const float3 m = normalize(wi + wo);
        const float ndoth = m.z;
        const float alpha2 = roughness * roughness;

        // ks is in the f term
        ShadowMaskTermSmith smith = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha2);

        const float  d = ndf_ggx(alpha2, ndoth);
        const float  g = smith.g2;
        const float3 f = eval_fresnel_schlick(F0, 1.0, dot(m, wi));

        result.pdf = pdf(wi, wo, m);
        result.value_over_pdf = f * smith.g2_over_g1_wo;
        result.value = d * f * g / (4.0 * wi.z *wo.z);
        result.transmission_ratio = 1.0.xxx - f;
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

struct DiffuseBrdf
{
    float3 reflectance;

    static DiffuseBrdf zero()
    {
        DiffuseBrdf result;
        result.reflectance = 0.0.xxx;
        return result;
    }

    float pdf()
    {
        return PI_RECIP_ONE;
    }

    BrdfSample sample(float3 wo, float2 urand)
    {
        BrdfSample result;
#if 0
		// sample diffuse ray using cosine-weighted hemisphere sampling 
        float3 sample_dir = sample_hemisphere(urand);
#else
        float phi = urand.x * TWO_PI;
        float cos_theta = sqrt(max(0.0, 1.0 - urand.y));
        float sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));

        float3 sample_dir = float3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
#endif

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
        result.pdf = pdf();
        result.value_over_pdf = reflectance;
        result.value = result.value_over_pdf * result.pdf;
        result.transmission_ratio = 0.0.xxx; // no meaning, ks is already in fresnel term.
        //result.weight = sample_weight;
        result.wi = sample_dir;
        return result;
    }

    BrdfResult eval(float3 wi)
    {
        if (wi.z <= 0.0)
        {
            return BrdfResult::invalid();
        }

        BrdfResult result;
        result.pdf = pdf();
        result.value_over_pdf = reflectance;
        result.value = result.value_over_pdf * result.pdf;
        result.transmission_ratio = 0.0.xxx; // no meaning, ks is already in fresnel term.
        return result;
    }
};

struct Brdf 
{
    DiffuseBrdf  diffuse_brdf;
    SpecularBrdf specular_brdf;

    static DiffuseBrdf metalness_to_diffuse_reflectance(float3 albedo, float metalness)
    {
        DiffuseBrdf diff = DiffuseBrdf::zero();
        diff.reflectance = albedo * (1.0 - metalness);

        float3 albedo_boost = metalness_albedo_boost(metalness, albedo);
        diff.reflectance = min(1.0, diff.reflectance * albedo_boost);

        return diff;
    }

    static SpecularBrdf metalness_to_specular_F0(float3 albedo, float metalness)
    {
        SpecularBrdf spec = SpecularBrdf::zero();
        spec.F0 = lerp(MIN_DIELECTRICS_REFLECTIVITY.xxx, albedo, metalness);

        float3 albedo_boost = metalness_albedo_boost(metalness, albedo);
        spec.F0 = min(1.0, spec.F0 * albedo_boost);

        return spec;
    }

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

    BrdfSample sample(float3 wo, float3 urand, in MultiScatterCompensate compensate)
    {
#if BRDF_FORCE_DIFFUSE_SAMPLE_ONLY
        return diffuse_brdf.sample(wo, urand.xy);
#endif

#if BRDF_FORCE_SPECULAR_SAMPLE_ONLY
        return specular_brdf.sample(wo, urand.xy);
#endif
        BrdfSample result;

        // In the layered material model, each surface is very thin and will transmit or reflect light.
        // The accumulated fresnel term determines the ratio of the energy that is transmit to the next surface. 
        // We should transmit with throughput equal to `BrdfResult.transmission_ratio`,
        // and reflect with the complement of that. 
        // However since we use a single ray, we toss a coin, and choose between reflection and transmission.
        // The result should be accumulate over frame.

        const float specular = rgb_color_to_luminance(compensate.preintegrated_reflection);
        const float diffuse = rgb_color_to_luminance(compensate.preintegrated_transmission_fraction * diffuse_brdf.reflectance);

        const float transmission_faction = diffuse / (diffuse + specular);

        const float lobe_xi = urand.z;
        if (lobe_xi < transmission_faction)
        {
            // transmission wins! Now sample the bottom layer (diffuse layer)
            result = diffuse_brdf.sample(wo, urand.xy);
            const float lobe_pdf = transmission_faction;

            // adjust pdf
            result.pdf *= lobe_pdf;
            result.value_over_pdf /= lobe_pdf;

            // account for the masking that the top level (specular) exerts on the bottom (diffuse).
            result.value_over_pdf *= compensate.preintegrated_transmission_fraction;
            result.value *= compensate.preintegrated_transmission_fraction;
        }
        else 
        {
            // reflection wins! we will not meet the diffuse layer.
            result = specular_brdf.sample(wo, urand.xy);
            const float lobe_pdf = (1.0 - transmission_faction);

            // adjust pdf
            result.value_over_pdf /= lobe_pdf;
            result.pdf *= lobe_pdf;

            // apply approximate multi-scatter energy preservation
            result.value_over_pdf *= compensate.preintegrated_reflection_multiplier;
            result.value *= compensate.preintegrated_reflection_multiplier;
        }

        return result;
    }

    float3 eval(float3 wi, float3 wo, in MultiScatterCompensate compensate)
    {
        if (wi.z <= 0 || wo.z <= 0)
        {
            return 0.0.xxx;
        }

        BrdfResult diff = diffuse_brdf.eval(wi);
        BrdfResult spec = specular_brdf.eval(wi, wo);

        return compensate.compensate_brdf(diff, spec, wo);
    }

    float3 eval_directional_light(float3 wi, float3 wo, in MultiScatterCompensate compensate)
    {
        if (wi.z <= 0 || wo.z <= 0)
        {
            return 0.0.xxx;
        }

        BrdfResult diff = diffuse_brdf.eval(wi);
        BrdfResult spec = specular_brdf.eval(wi, wo);

        return compensate.compensate_brdf_directional_light(diff, spec, wi, wo);
    }
};

#endif