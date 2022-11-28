#include "../brdf.hlsl"

#include "../../math/math.hlsl"
#include "../../math/constants.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

[[vk::binding(0)]] RWTexture2D<float2> output_tex;

float2 integrate_brdf(float ndotv, float roughness) {
    float3 wo = float3(sqrt(1.0 - ndotv * ndotv), 0, ndotv);

    float scale = 0.0;
    float bias = 0.0;

    const float alpha = roughness;

    static const uint num_samples = 1024;
    for (uint i = 0; i < num_samples; ++i) {
        float2 urand = hammersley(i, num_samples);

        float3 Gm = SpecularBrdf::sample_ggx_vndf(wo, alpha, urand);

        // Reflect view direction to obtain light vectors
	    const float3 wi = reflect(-wo, Gm);

        bool is_valid = true;
        // invalid sample direction
        if (Gm.z <= 1e-6 || wi.z <= 1e-6 || wo.z <= 1e-6) {
			is_valid = false;
		}

        if (is_valid)
        {
            // Fresnel term is always 1.0
            float a = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha * alpha).g2_over_g1_wo * 1.0;
            // multiply by a Fc term in https://learnopengl.com/PBR/IBL/Specular-IBL
            float Fc = pow(1.0 - dot(Gm, wi), 5.0);
            float b = ShadowMaskTermSmith::eval(wo.z, wi.z, alpha * alpha).g2_over_g1_wo * Fc;

            scale += a - b;
            bias  += b;
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
