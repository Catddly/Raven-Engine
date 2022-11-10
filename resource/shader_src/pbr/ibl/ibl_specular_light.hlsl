#include "../../math/constants.hlsl"
#include "../../math/math.hlsl"
#include "../../math/coordinate.hlsl"
#include "../../common/uv.hlsl"
#include "../brdf.hlsl"

#include "../../common/immutable_sampler.hlsl"

[[vk::push_constant]]
struct {
    uint render_res;
    uint prefilter_res;
} push_constants;

[[vk::binding(0)]] TextureCube cube_map;
[[vk::binding(1)]] RWTexture2DArray<float4> prefilter_cube_map;

#define ENABLE_LOW_SAMPLES_PREFILTER_MAP 1

float3 get_env_radiance(float3 dir, float level)
{
    return cube_map.SampleLevel(sampler_llce, dir, level).rgb;
}

// generate sample vectors in some region constrained by the roughness oriented around the microfacet's halfway vector.
float3 importance_sample_ggx(float2 urand, float3 N, float alpha)
{
    float phi = 2.0 * PI * urand.x;
    float cos_theta = sqrt((1.0 - urand.y) / (1.0 + (alpha * alpha - 1.0) * urand.y));
    float sin_theta = sqrt(1.0 - cos_theta * cos_theta);
	
    // From spherical coordinates to cartesian coordinates
    float3 H = float3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
	
    // From tangent-space vector to world-space sample vector
    const float3x3 tangent_to_world = build_orthonormal_basis(N);

    float3 sampleDir = mul(tangent_to_world, H);
    return normalize(sampleDir);
}

float3 get_prefilter_radiance(float3 N, float roughness)
{
    float3 R = N;
    float3 V = R;
    
    float  total_weight = 0.0;
    float3 prefilter_color = 0.0.xxx;
    
    const float alpha = roughness * roughness;
    const int sample_count = 128;

    // Generate sampleCount number of a low discrepancy random directions in the 
    // specular lobe and add the environment map data into a weighted sum.
    for (int i = 0; i < sample_count; i++)
    {
        float2 rand_hemisphere = hammersley(i, sample_count);
        float3 H = importance_sample_ggx(rand_hemisphere, N, alpha);
        float3 L = normalize(2.0 * dot(V, H) * H - V);

        float ndotl = max(dot(N, L), 1e-5);

        if (ndotl > 0.0)
        {
            float level = 0.0;

#if ENABLE_LOW_SAMPLES_PREFILTER_MAP
            // Sample the mip levels of the environment map
            // From https://placeholderart.wordpress.com/2015/07/28/implementation-notes-runtime-environment-map-filtering-for-image-based-lighting/
            // Vectors to evaluate pdf
            float ndoth = saturate(dot(N, H));
            float vdoth = saturate(dot(V, H));

            float pdf = SpecularBrdf::ndf_ggx(alpha * alpha, ndoth) * ndoth / (4.0 * vdoth);

            // Solid angle represented by this sample
            float omega_s = 1.0 / (float(sample_count) * pdf);

            float cubemap_size = float(push_constants.render_res);
            // Solid angle covered by 1 pixel
            float omega_p = 4.0 * PI / (6.0 * cubemap_size * cubemap_size);
            // Original paper suggests biasing the mip to improve the results
            float mip_bias = 1.0;
            level = max(0.5 * log2(omega_s / omega_p) + mip_bias, 0.0);
#endif

            prefilter_color += get_env_radiance(L, level) * ndotl;
            total_weight    += ndotl;
        }
    }

    prefilter_color = prefilter_color / total_weight;
    return prefilter_color;
}

[numthreads(8, 8, 6)]
void main(in uint3 px: SV_DispatchThreadID) {
    if (px.x >= push_constants.prefilter_res || px.y >= push_constants.prefilter_res) {
        return;
    }

    float max_mipmap_level = log2(float(push_constants.render_res));
    float mip_level = max_mipmap_level - log2(float(push_constants.prefilter_res));
    float roughness = mip_level / max_mipmap_level;

    float3 dir = normalize(cube_to_world_dir(int3(px), float2(push_constants.prefilter_res, push_constants.prefilter_res)));

    if (push_constants.render_res == push_constants.prefilter_res) // mipmap level 0, do not need to filter the env map, just do copy
    {
        float3 radiance = get_env_radiance(dir, 0.0);
        prefilter_cube_map[px] = float4(radiance, 1.0);
    }
    else
    {
        float3 prefiltered_radiance = get_prefilter_radiance(dir, roughness);
        prefilter_cube_map[px] = float4(prefiltered_radiance, 1.0);
    }
}