#include "../brdf.hlsl"

#include "../../math/math.hlsl"
#include "../../math/constants.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

[[vk::binding(0)]] RWTexture2D<float2> output_tex;

// split into F0 * scale + bias
float2 integrate_brdf(float ndotv, float roughness)
{
    float3 wo = float3(sqrt(1.0 - ndotv * ndotv), 0, ndotv);

    float scale = 0.0;
    float bias = 0.0;

    SpecularBrdf brdf_no_fresnel;
    brdf_no_fresnel.roughness = roughness;
    // force F0 to be 1.0, then the fresnel function will always be 1.0
    brdf_no_fresnel.F0 = 1.0.xxx;

    SpecularBrdf brdf_pow_5 = brdf_no_fresnel;
    // force F0 to be 0.0, then the fresnel function will always be the 
    // later part (pow(max(0.0, 1.0 - ndots), 5.0))
    brdf_pow_5.F0 = 0.0;

    const float alpha = roughness;

    static const uint num_samples = 1024;
    for (uint i = 0; i < num_samples; ++i)
    {
        float2 urand = hammersley(i, num_samples);

        // result.value_over_pdf wil be g_term.g2_over_g1_wo;
        BrdfSample sample_no_fresnel = brdf_no_fresnel.sample(wo, urand);

        if (sample_no_fresnel.is_valid())
        {
            // // Fresnel term is always 1.0
            // float a = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha * alpha).g2_over_g1_wo * 1.0;
            // // multiply by a Fc term in https://learnopengl.com/PBR/IBL/Specular-IBL
            // float Fc = pow(1.0 - dot(Gm, wi), 5.0);
            // float b = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha * alpha).g2_over_g1_wo * Fc;

            BrdfResult result_pow_5 = brdf_pow_5.eval(sample_no_fresnel.wi, wo);

            // See https://learnopengl.com/PBR/IBL/Specular-IBL for the equation
            scale += (sample_no_fresnel.value_over_pdf.x - result_pow_5.value_over_pdf.x);
            bias += result_pow_5.value_over_pdf.x;
        }
    }

    return float2(scale, bias) / num_samples;
}

[numthreads(8, 8, 1)]
void main(in uint2 px : SV_DispatchThreadID) 
{
    // with some bias to get the correct result
    float ndotv = (px.x / (push_constants.render_res_width - 1.0)) * (1.0 - 1e-3) + 1e-3;
    float roughness = max(1e-5, px.y / (push_constants.render_res_height - 1.0));

    output_tex[px] = integrate_brdf(ndotv, roughness);
}
