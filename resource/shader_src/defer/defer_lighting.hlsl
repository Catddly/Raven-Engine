#include "gbuffer.hlsl"
#include "../math/constants.hlsl"
#include "../math/math.hlsl"
#include "../common/frame_constants.hlsl"
#include "../common/float_precision.hlsl"
#include "../common/uv.hlsl"
#include "../common/immutable_sampler.hlsl"

#include "../ray_tracing/ray.hlsl"
#include "../ray_tracing/camera_ray.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

[[vk::binding(0)]] Texture2D<float4> gbuffer_tex;
[[vk::binding(1)]] Texture2D<float> depth_tex;
[[vk::binding(2)]] RWTexture2D<float4> output_tex;
[[vk::binding(3)]] TextureCube cube_map;
[[vk::binding(4)]] TextureCube convolved_cube_map;
[[vk::binding(5)]] TextureCube prefilter_cube_map;
[[vk::binding(6)]] Texture2D<float2> brdf_lut;

#define CONVOLVED_CUBEMAP convolved_cube_map
#define PREFILTERED_CUBEMAP prefilter_cube_map
#define BRDF_LUT brdf_lut

#include "../pbr/multi_scatter_compensate.hlsl"
#include "../pbr/ibl/ibl_lighting.hlsl"
#include "../pbr/brdf.hlsl"

// https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/
float3 ACESFilm(float3 x){
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

[numthreads(8, 8, 1)]
void main(in uint2 px: SV_DispatchThreadID) {
    float2 resolution = float2(push_constants.render_res_width, push_constants.render_res_height);
    float2 uv = pixel_to_uv(float2(px), resolution);

    CameraRayContext cam_ctx = CameraRayContext::from_screen_uv(uv);

    const float depth = depth_tex[px];
    // draw environment map on depth 0.0 (infinite far away)
    if (depth - 0.0 < FLOAT_EPSILON)
    {
        float3 direction = cam_ctx.get_direction_ws();
        float4 pixel = cube_map.SampleLevel(sampler_llce, direction, 0);

        output_tex[px] = float4(pixel.rgb, 1.0);
        return;
    }

    RayDesc view_ray = new_ray(
        cam_ctx.get_position_ws(),
        cam_ctx.get_direction_ws(),
        0.0,
        FLOAT_MAX
    );

    GBuffer gbuffer = PackedGBuffer::from_uint4(asuint(gbuffer_tex[px])).unpack();

    // tmeporary
    const float3 SUN_DIRECTION = normalize(float3(-0.32803, 0.90599, 0.26749));

    // Build a orthonormal basis that transform tangent space vector to world space.
    // Notice that during multiplication we put the vector on the right side of the mul(),
    // this is equivalent to multiply a transpose matrix.
    // And the matrix is a orthogonal matrix, its transpose matrix is also its inverse matrix.
    // So the multiplication is equivalent to transform the vector from world space to tangent space.
    const float3x3 tangent_to_world = build_orthonormal_basis(gbuffer.normal);
    // incoming light solid angle in tangent space
    // because we store the normal in the z column of the matrix, so wi.z is the dot product of normal and light.
    const float3 wi = normalize(mul(SUN_DIRECTION, tangent_to_world));
    // outcoming light solid angle in tangent space
    // Ibid.
    // wo.z is the dot product of normal and view.
    float3 wo = normalize(mul(-view_ray.Direction, tangent_to_world));

    // if (wo.z < 0.0) {
    //     wo.z *= -0.25;
    //     wo = normalize(wo);
    // }

    Brdf brdf = Brdf::from_gbuffer(gbuffer, wo.z);
    MultiScatterCompensate compensate = MultiScatterCompensate::compensate_for(wo, gbuffer.roughness, brdf.specular_brdf.F0);

    float3 total_radiance = 0.0.xxx;
    // direct lighting
    {
        const float3 brdf_value = brdf.eval_ndotl_weighted_direction_light(wi, wo, compensate); // ndotl term is already in brdf
        const float3 light_radiance = float3(1.0, 1.0, 1.0);

        total_radiance += brdf_value * light_radiance;
    }
    
    // indirect lighting
    {
        Ibl ibl = Ibl::from_brdf(brdf.specular_brdf);
        const float3 R = reflect(view_ray.Direction, gbuffer.normal);

        float3 irradiance = ibl.eval_gbuffer(gbuffer, wo, R, compensate, brdf.diffuse_brdf.reflectance);

        total_radiance += irradiance;
    }

    // tone mapping
    total_radiance = ACESFilm(total_radiance);

    // gamma correction
    total_radiance = pow(total_radiance, 1.0 / 2.2);

    output_tex[px] = float4(total_radiance, 1.0);
}