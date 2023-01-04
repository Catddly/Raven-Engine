#include "gbuffer.hlsl"
#include "../math/constants.hlsl"
#include "../math/math.hlsl"
#include "../color/color_space.hlsl"
#include "../common/frame_constants.hlsl"
#include "../common/float_precision.hlsl"
#include "../common/uv.hlsl"
#include "../common/immutable_sampler.hlsl"
#include "../common/bindless_resources.hlsl"
#include "../light/light_commons.hlsl"

#include "../ray_tracing/ray.hlsl"
#include "../ray_tracing/camera_ray.hlsl"

[[vk::push_constant]]
struct {
    uint render_res_width;
    uint render_res_height;
} push_constants;

struct SHBuffer
{
    float red_coeffs[9];
    float green_coeffs[9];
    float blue_coeffs[9];
};

[[vk::binding(0)]] Texture2D<float4> gbuffer_tex;
[[vk::binding(1)]] Texture2D<float> depth_tex;
[[vk::binding(2)]] RWTexture2D<float4> output_tex;
[[vk::binding(3)]] Texture2D<float> light_map[MAX_DIRECTIONAL_LIGHT_COUNT];
[[vk::binding(4)]] StructuredBuffer<row_major float4x4> light_map_transforms_dyn; // TODO: maybe move this to frame constants?
[[vk::binding(5)]] TextureCube cube_map;
[[vk::binding(6)]] StructuredBuffer<SHBuffer> sh_buffer;
[[vk::binding(7)]] TextureCube prefilter_cube_map;

#define CONVOLVED_CUBEMAP convolved_cube_map
#define PREFILTERED_CUBEMAP prefilter_cube_map
#define SH_BUFFER sh_buffer

#include "../pbr/brdf.hlsl"
#include "../pbr/ibl/ibl_lighting.hlsl"
#include "../pbr/multi_scatter_compensate.hlsl"

// Note: bias matrix to move NDC (coord x and y) [-1, 1] to [0, 1] for texture sampling
// inverse y here to compensate the negative y viewport in vulkan (see ctx.set_viewport())
static const float4x4 bias_mat = float4x4(
    0.5,  0.0, 0.0, 0.5,
    0.0, -0.5, 0.0, 0.5,
    0.0,  0.0, 1.0, 0.0,
    0.0,  0.0, 0.0, 1.0
);

float is_shadowed(uint light_index, float3 position_ws)
{
    float4 shadow_coord = mul(bias_mat, mul(light_map_transforms_dyn[light_index], float4(position_ws, 1.0)));
    // Note: notice that orthographic projection is linear transform therefore keep the w as 1.0,
    // but for interoperability, we keep the perspective division here.
    // perspective division (i.e. homogeneous clipping space divide w)
    shadow_coord /= shadow_coord.w;

    float shadowed = 1.0;

    // outside the depth range is all shadowed
    if (shadow_coord.z >= 0.0 && shadow_coord.z <= 1.0)
	{
		const float closest_depth = light_map[light_index].SampleLevel(sampler_lnce, shadow_coord.xy, 0.0).r;

		if (shadow_coord.w > 0.0 && closest_depth < shadow_coord.z)
		{
			shadowed = 0.0;
		}
	}

    return shadowed;
}

[numthreads(8, 8, 1)]
void main(in uint2 px: SV_DispatchThreadID)
{
    float2 resolution = float2(push_constants.render_res_width, push_constants.render_res_height);
    float2 uv = pixel_to_uv(float2(px), resolution);

    CameraRayContext cam_ctx = CameraRayContext::from_screen_uv(uv);

    const float depth = depth_tex[px];
    // draw environment map on depth 0.0 (infinite far away)
    if (depth - 0.0 < FLOAT_EPSILON)
    {
        float3 direction = cam_ctx.get_direction_ws();

        float4 pixel = cube_map.SampleLevel(sampler_llce, direction, 0.0);
        output_tex[px] = float4(srgb_to_linear(pixel.rgb) * frame_constants_dyn.pre_exposure_mult, 1.0);
        return;
    }

    RayDesc view_ray = new_ray(
        cam_ctx.get_position_ws(),
        cam_ctx.get_direction_ws(),
        0.0,
        FLOAT_MAX
    );

    GBuffer gbuffer = PackedGBuffer::from_uint4(asuint(gbuffer_tex[px])).unpack();

    // Build a orthonormal basis that transform tangent space vector to world space.
    // Notice that during multiplication we put the vector on the right side of the mul(),
    // this is equivalent to multiply a transpose matrix.
    // And the matrix is a orthogonal matrix, its transpose matrix is also its inverse matrix.
    // So the multiplication is equivalent to transform the vector from world space to tangent space.
    const float3x3 tangent_to_world = build_orthonormal_basis(gbuffer.normal);
    // outcoming light solid angle in tangent space
    // because we store the normal in the z column of the matrix, so wo.z is the dot product of normal and view.
    float3 wo = mul(-view_ray.Direction, tangent_to_world);

    Brdf brdf = Brdf::from_gbuffer(gbuffer);
    MultiScatterCompensate compensate = MultiScatterCompensate::compensate_for(wo, gbuffer.roughness, brdf.specular_brdf.F0);

    const float3 pos_ws = cam_ctx.get_frag_position_ws(depth);

    float3 total_radiance = 0.0.xxx;
    // direct lighting
    {
        uint directional_shadow_map_index = 0;
        for (uint i = 0; i < frame_constants_dyn.directional_light_count; ++i)
        {
            // TODO: add some utility function
            const LightFrameConstants light = frame_constants_dyn.light_constants[i];
            
            // incoming light solid angle in tangent space
            // Ibid.
            // wi.z is the dot product of normal and light.
            const float3 wi = mul(normalize(light.direction), tangent_to_world);

            const float3 brdf_value = brdf.eval_directional_light(wi, wo, compensate);
            const float3 light_radiance = light.color * light.intensity;

            float shadowed = 0.0;
            if (light.shadowed)
            {
                shadowed = is_shadowed(directional_shadow_map_index, pos_ws);
                directional_shadow_map_index += 1;
            }
            total_radiance += (1.0 - shadowed) * (brdf_value * light_radiance * max(0.0, wi.z));
        }
    }

    // indirect lighting
    {
        Ibl ibl = Ibl::from_brdf(brdf.specular_brdf); 
        const float3 R = reflect(view_ray.Direction, gbuffer.normal);

        float3 irradiance = ibl.eval_gbuffer(gbuffer, wo, R, compensate, brdf.diffuse_brdf.reflectance);

        total_radiance += irradiance;
    }

    total_radiance *= frame_constants_dyn.pre_exposure_mult;

    output_tex[px] = float4(total_radiance, 1.0);
}