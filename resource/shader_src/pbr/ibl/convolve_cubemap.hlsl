#include "../../math/constants.hlsl"
#include "../../math/math.hlsl"
#include "../../math/coordinate.hlsl"

#include "../../common/immutable_sampler.hlsl"

#include "../../ray_tracing/camera_ray.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

[[vk::binding(0)]] TextureCube cube_map;
[[vk::binding(1)]] RWTexture2DArray<float4> convolve_cube_map;

[numthreads(8, 8, 1)]
void main(in uint3 px: SV_DispatchThreadID) {
    const float3x3 CUBE_MAP_FACE_ROTATIONS[6] = {
        float3x3(0,0,1, 0,-1,0, -1,0,0),   // right
        float3x3(0,0,-1, 0,-1,0, 1,0,0),     // left

        float3x3(1,0,0, 0,0,1, 0,1,0),     // top
        float3x3(1,0,0, 0,0,-1, 0,-1,0),     // bottom

        float3x3(1,0,0, 0,-1,0, 0,0,1),    // back
        float3x3(-1,0,0, 0,-1,0, 0,0,-1),    // front
    };

    uint face = px.z;
    float2 uv = pixel_to_uv(px.xy, float2(push_constants.render_res_width, push_constants.render_res_height));

    float3 output_dir = normalize(mul(CUBE_MAP_FACE_ROTATIONS[face], float3(uv * 2 - 1, 1.0)));
    const float3x3 cs_dir_to_world = build_orthonormal_basis(output_dir);

    float sample_delta = 0.025;
    float num_samples = 0.0;

    float3 irradiance = 0.0;
    // on hemisphere per solid angle
    for(float theta = 0.0; theta < 2.0 * PI; theta += sample_delta)
    {
        for(float phi = 0.0; phi < 0.5 * PI; phi += sample_delta)
        {
            float3 sample_point = spherical_to_cartesian_unit_sphere(theta, phi);
            float3 sample_dir = mul(cs_dir_to_world, sample_point); 

            irradiance += cube_map.SampleLevel(sampler_llr, sample_dir, 0).rgb * cos(phi) * sin(phi);
            num_samples += 1.0;
        }
    }

    irradiance = PI * irradiance * (1.0 / num_samples);
    convolve_cube_map[px] = float4(irradiance, 1.0);
}