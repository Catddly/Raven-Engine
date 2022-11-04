#include "gbuffer.hlsl"
#include "../math/constants.hlsl"
#include "../math/math.hlsl"
#include "../common/frame_constants.hlsl"
#include "../common/float_precision.hlsl"
#include "../common/uv.hlsl"
#include "../common/immutable_sampler.hlsl"

#include "../ray_tracing/ray.hlsl"
#include "../ray_tracing/camera_ray.hlsl"

#include "../pbr/brdf.hlsl"

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

static const float2 BRDF_FG_LUT_UV_SCALE = (512 - 1.0) / 512;
static const float2 BRDF_FG_LUT_UV_BIAS = 0.5.xx / 512;

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

    // float4 cs_pos = float4(cs_coord, depth, 1.0);
    // float4 ws_pos = mul(cam.view_to_world, mul(cam.clip_to_view, cs_pos));
    // ws_pos /= ws_pos.w;

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

    //float3 R = reflect(view_ray.Direction, gbuffer.normal);

    // calculate lighting
    
    // direct lighting
    float3 total_radiance = 0.0.xxx;

    Brdf brdf = Brdf::from_gbuffer(gbuffer);
    const float3 brdf_value = brdf.eval_ndotl_weighted(wi, wo); // ndotl term is already in brdf
    const float3 light_radiance = float3(1.0, 1.0, 1.0);

    total_radiance += brdf_value * light_radiance;

    // indirect lighting
    // const float3 normal = gbuffer.normal;
    // const float3 ks = fresnel_schlick_roughness(max(wo.z, 0.0), brdf.specular_brdf.albedo, brdf.specular_brdf.roughness);
    // float4 diff_irradiance_sample = convolved_cube_map.SampleLevel(sampler_llce, normal, 0);
    // float3 kd = (1.0.xxx - ks) * (1.0 - gbuffer.metalness);
    // float3 diffuse_ibl = diff_irradiance_sample.rgb * gbuffer.albedo;

    // float3 prefiltered_color = prefilter_cube_map.SampleLevel(sampler_llce, R, 0).rgb;
    // float2 brdf_lut_value = brdf_lut.SampleLevel(sampler_lnce, float2(max(wo.z, 0.0), gbuffer.roughness) * BRDF_FG_LUT_UV_SCALE + BRDF_FG_LUT_UV_BIAS, 0);
    // float3 specular = prefiltered_color * (brdf.specular_brdf.albedo * brdf_lut_value.x + brdf_lut_value.y);

    //total_radiance += kd * diffuse_ibl + specular;

    //total_radiance = kd * diffuse_ibl * 6.0;
    output_tex[px] = float4(total_radiance, 1.0);
    
}