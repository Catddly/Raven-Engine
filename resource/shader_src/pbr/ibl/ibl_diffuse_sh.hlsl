#include "../../common/uv.hlsl"
#include "../../math/math.hlsl"
#include "../../math/coordinate.hlsl"

#include "../../common/immutable_sampler.hlsl"
#include "../../math/spherical_harmonics.hlsl"

struct SHBuffer
{
    float red_coeffs[9];
    float green_coeffs[9];
    float blue_coeffs[9];
};

[[vk::binding(0)]] Texture2DArray<float4> cube_map;
[[vk::binding(1)]] RWStructuredBuffer<SHBuffer> sh_buffer;

float3 get_env_radiance(float3 location)
{
    return cube_map.SampleLevel(sampler_llce, location, 0.0).rgb;
}

[numthreads(8, 8, 1)]
void main(uint3 px: SV_DispatchThreadID)
{
    // Run only once
    if (any(px >= 1))
    {
        return;
    }

    const uint num_samples = 2048;
    
    SphericalHarmonicsBasis red_basis = SphericalHarmonicsBasis::zero();
    SphericalHarmonicsBasis green_basis = SphericalHarmonicsBasis::zero();
    SphericalHarmonicsBasis blue_basis = SphericalHarmonicsBasis::zero();

    for (int i = 0; i < num_samples; ++i)
    {
        // Used a spiral spherical fibonacci sequences to sample uniform directions on the sphere
        float3 dir = spherical_fibonacci(i, num_samples);
        float3 radiance = get_env_radiance(dir);

        SphericalHarmonicsBasis dir_basis = SphericalHarmonicsBasis::from_direction(dir);

        red_basis = red_basis.add(dir_basis.mul_scaler(radiance.r));
        green_basis = green_basis.add(dir_basis.mul_scaler(radiance.g));
        blue_basis = blue_basis.add(dir_basis.mul_scaler(radiance.b));
    }

    const float normalized_factor = FOUR_PI / (float)(num_samples);

    red_basis = red_basis.mul_scaler(normalized_factor);
    green_basis = green_basis.mul_scaler(normalized_factor);
    blue_basis = blue_basis.mul_scaler(normalized_factor);

    store_sh(sh_buffer[0].red_coeffs, red_basis);
    store_sh(sh_buffer[0].green_coeffs, green_basis);
    store_sh(sh_buffer[0].blue_coeffs, blue_basis);
}