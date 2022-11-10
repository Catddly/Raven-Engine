#include "../../math/constants.hlsl"
#include "../../math/math.hlsl"
#include "../../math/coordinate.hlsl"
#include "../../common/uv.hlsl"

[[vk::push_constant]]
struct {
    uint render_res;
    uint mipmap_res;
} push_constants;

[[vk::binding(0)]] Texture2DArray<float4> cube_map;
[[vk::binding(1)]] RWTexture2DArray<float4> convolve_cube_map;

float3 get_env_radiance(int4 location)
{
    return cube_map.Load(location).rgb;
}

[numthreads(8, 8, 6)]
void main(in uint3 px: SV_DispatchThreadID) 
{
    if (px.x >= push_constants.mipmap_res || px.y >= push_constants.mipmap_res) {
        return;
    }

    float3 dir = normalize(cube_to_world_dir(int3(px), float2(push_constants.mipmap_res, push_constants.mipmap_res)));
    const float3x3 cs_dir_to_world = build_orthonormal_basis(dir);

    float sample_delta = 0.025;
    float num_samples = 0.0;

    float3 irradiance = 0.0;
    // do intergral over hemisphere per solid angle
    for(float phi = 0.0; phi < 2.0 * PI; phi += sample_delta)
    {
        for(float theta = 0.0; theta < 0.5 * PI; theta += sample_delta)
        {
            float3 sample_point = spherical_to_cartesian_unit_sphere(theta, phi);
            // tangent space to world
            float3 sampleVec = mul(cs_dir_to_world, sample_point);
            // world to cube coord
            int3 location = tex_coord_to_cube(sampleVec, float2(push_constants.render_res, push_constants.render_res));
            float3 radiance = get_env_radiance(int4(location, 0));

            irradiance += radiance * cos(theta) * sin(theta);
            num_samples += 1.0;
        }
    }

    irradiance = PI * irradiance * (1.0 / num_samples);

    convolve_cube_map[px] = float4(irradiance, 1.0);
}