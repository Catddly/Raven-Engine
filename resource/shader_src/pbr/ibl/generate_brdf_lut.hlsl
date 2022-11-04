#include "../brdf.hlsl"

#include "../../math/constants.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

[[vk::binding(0)]] RWTexture2D<float2> output_tex;

float radical_inverse_vdc(uint bits) {
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return float(bits) * 2.3283064365386963e-10; // / 0x100000000
}

float2 hammersley(uint i, uint n) {
    return float2(float(i + 1) / n, radical_inverse_vdc(i + 1));
}

float2 integrate_brdf(float roughness, float ndotv) {
    float3 wo = float3(sqrt(1.0 - ndotv * ndotv), 0, ndotv);

    float a = 0;
    float b = 0;

    SpecularBrdf brdf_a;
    brdf_a.roughness = roughness;
    brdf_a.F0 = 1.0.xxx;

    SpecularBrdf brdf_b = brdf_a;
    brdf_b.F0 = 0.0;

    static const uint num_samples = 1024;
    for (uint i = 0; i < num_samples; ++i) {
        float2 urand = hammersley(i, num_samples);
        BrdfSample v_a = brdf_a.sample(wo, urand);
        //BrdfResult v_a_res = brdf_a.eval_ndotl_weighted(v_a.wi, wo);

        if (v_a.is_valid()) {
            BrdfResult v_b = brdf_b.eval_ndotl_weighted(v_a.wi, wo);

            a += (v_a.weight.x - v_b.value_over_pdf.x);
            b += v_b.value_over_pdf.x;
        }
    }

    return float2(a, b) / num_samples;
}

[numthreads(8, 8, 1)]
void main(in uint2 pix : SV_DispatchThreadID) {
    float ndotv = (pix.x / (push_constants.render_res_width - 1.0)) * (1.0 - 1e-3) + 1e-3;
    float roughness = max(1e-5, pix.y / (push_constants.render_res_height - 1.0));

    output_tex[pix] = integrate_brdf(roughness, ndotv);
}
