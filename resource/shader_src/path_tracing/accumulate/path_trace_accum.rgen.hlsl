#include "../../common/hash.hlsl"
#include "../../common/frame_constants.hlsl"
#include "../../common/immutable_sampler.hlsl"
#include "../../color/color_space.hlsl"
#include "../../math/constants.hlsl"
#include "../../math/math.hlsl"

#include "../../ray_tracing/camera_ray.hlsl"
#include "../../ray_tracing/ray.hlsl"
#include "../../ray_tracing/ray_cone.hlsl"
#include "../../ray_tracing/gbuffer_ray_payload.hlsl"
#include "../../ray_tracing/gbuffer_ray_tracing.hlsl"
#include "../../ray_tracing/ray_tracing_shadow.hlsl"

[[vk::binding(0)]] RaytracingAccelerationStructure tlas;
// w channel in output_tex is the accumulated time.
[[vk::binding(1)]] RWTexture2D<float4> output_tex;
[[vk::binding(2)]] TextureCube env_map;

static const uint  MAX_PATH_TRACING_LENGTH = 16;
// used to restrict the secondary ray hit range
static const float MAX_SECONDARY_RAY_LENGTH = FLOAT_MAX;

static const uint RUSSIAN_ROULETTE_START_PATH_INDEX = 3;

static const float MAX_PIXEL_ACCUMULATETION = 1000.0;
static const uint  SAMPLE_COUNT = 1; // one ray one sample now

#define BRDF_LUT brdf_lut

#include "../../pbr/brdf.hlsl"
#include "../../pbr/multi_scatter_compensate.hlsl"

float3 sample_env(float3 direction)
{
    float3 env_light = env_map.SampleLevel(sampler_lnce, direction, 0.0).rgb;
    return srgb_to_linear(env_light);
}

[shader("raygeneration")]
void main()
{
    // get current pixel index
    const uint2 px = DispatchRaysIndex().xy;

    // load prev frame's output value
    float4 prev_px = output_tex[px];

    // path trace hard shadow
    // temporary
    const float3 SUN_DIRECTION = normalize(float3(-0.32803, 0.90599, 0.26749));

    if (prev_px.w < MAX_PIXEL_ACCUMULATETION)
    {
        float4 output_packed_total_radiance = 0.0;
        uint rng = hash_combine3(px.x, px.y, frame_constants_dyn.frame_index);

        for (uint sample_index = 0; sample_index < SAMPLE_COUNT; ++sample_index)
        {
            // 1. Calculate eye ray.
            float2 pixel_offset = float2(0.5, 0.5);

            const float2 px_center = float2(px) + pixel_offset;
            const float2 screen_uv = px_center / DispatchRaysDimensions().xy;

            const CameraRayContext cam_ray = CameraRayContext::from_screen_uv(screen_uv);
            RayDesc sample_ray = new_ray(
                cam_ray.get_position_ws(),
                normalize(cam_ray.get_direction_ws()),
                0.0,
                FLOAT_MAX
            );

            RayCone ray_cone = RayCone::from_image_height(DispatchRaysDimensions().y);
            // sharpen the texture
            // the less the spread angle, the less light can be seen by this pixel and the lod is smaller
            ray_cone.spread_angle *= 0.3;

            float3 throughput = 1.0.xxx; // represent the energy transmission ratio
            float3 total_radiance = 0.0.xxx;

            // 2. Sample the ray along the path.
            [loop]
            for (uint path_index = 0; path_index < MAX_PATH_TRACING_LENGTH; ++path_index)
            {
                if (path_index == 1)
                {
                    sample_ray.TMax = MAX_SECONDARY_RAY_LENGTH;
                }

                // trace primary ray
                // TODO: optimize, the first ray calculation can used rasterization gbuffer.
                RayTracingGBuffer gbuffer = RayTracingGBuffer::from_sample_ray(sample_ray)
                    .with_ray_cone(ray_cone)
                    .with_path_index(path_index)
                    .with_back_face_culling(false);

                GBufferHitPoint primary_hit = gbuffer.trace(tlas);

                if (primary_hit.is_hit)
                {
                    // TODO: this will assume that all hit surface is planar
                    // See Paper[1] in ray_cone.hlsl
                    const float subsequent_spread_angle = 0.0;
                    ray_cone = ray_cone.propagate(primary_hit.t, subsequent_spread_angle);

                    // only trace shadow for primary ray
                    bool is_shadowed = (path_index == 0) && ray_trace_shadow_directional(tlas, primary_hit.position, -SUN_DIRECTION);

                    GBuffer gbuffer_data = primary_hit.packed_gbuffer.unpack();

                    // if we hit the back face of a triangle
                    if (dot(gbuffer_data.normal, sample_ray.Direction) >= 0.0)
                    {
                        if (path_index == 0)
                        {
                            // flip the normal for primary hits so we don't see blackness
                            gbuffer_data.normal = -gbuffer_data.normal;
                        }
                        else // terminate path trace
                        {
                            break;
                        }
                    }

                    // 3. Eval lighting for this gbuffer data
                    const float3x3 tangent_to_world = build_orthonormal_basis(gbuffer_data.normal);
                    // right multiply tangent_to_world matrix to apply world to tangent space transformation.
                    // pretty much the same in defer/defer_lighting.hlsl
                    const float3 wi = mul(SUN_DIRECTION, tangent_to_world);
                    float3 wo = mul(-sample_ray.Direction, tangent_to_world);

                    Brdf brdf = Brdf::from_gbuffer(gbuffer_data);
                    MultiScatterCompensate compensate = MultiScatterCompensate::compensate_for(wo, gbuffer_data.roughness, brdf.specular_brdf.F0);

                    const float3 brdf_value = brdf.eval_directional_light(wi, wo, compensate);
                    const float3 light_radiance = is_shadowed ? 0.0.xxx : float3(1.0, 1.0, 1.0);
            
                    total_radiance += throughput * brdf_value * light_radiance * max(0.0, wi.z);

                    // 4. Sample ray to continue path tracing
                    float3 urand = float3(
                        uint_to_u01_float(hash1_mut(rng)),
                        uint_to_u01_float(hash1_mut(rng)),
                        uint_to_u01_float(hash1_mut(rng))
                    );
                    BrdfSample brdf_sample = BrdfSample::invalid();

                    // sample to get the next ray direction
                    brdf_sample = brdf.sample(wo, urand, compensate);

                    if (brdf_sample.is_valid())
                    {
                        // build next sample ray
                        sample_ray.Origin = primary_hit.position;
                        sample_ray.Direction = mul(tangent_to_world, brdf_sample.wi);
                        sample_ray.TMin = 1e-4;

                        // Remember to multiply the value over pdf!
                        // value over pdf is the Monte Carlo Estimator weight of this sample.
                        // Multiply it to get the current contribution to final accumulated lighting.
                        throughput *= brdf_sample.value_over_pdf;
                    }
                    else
                    {
                        break;
                    }

                    // perform a russian roulette when the path trace sample is high enough
                    // this pruning operation will have better performance
                    if (path_index >= RUSSIAN_ROULETTE_START_PATH_INDEX)
                    {
                        // toss a coin
                        const float coin = uint_to_u01_float(hash1_mut(rng));

                        // try more paths on the hit surface which is darker (i.e. albedo is smaller)
                        const float continue_p = max(gbuffer_data.albedo.r, max(gbuffer_data.albedo.g, gbuffer_data.albedo.b));
                        if (coin > continue_p)
                        {
                            break;
                        }
                        else
                        {
                            throughput /= continue_p;
                        }
                    }
                }
                else // sample sky
                {
                    total_radiance += throughput * sample_env(sample_ray.Direction);
                    // terminate the ray path
                    break;
                }
            }

            // finish path trace
            if (all(total_radiance >= 0.0)) // prevent undefined negative radiance value
            {
                output_packed_total_radiance += float4(total_radiance, 1.0); // w component here is the number of sample times
            }
        }

        // finish all samples in one single pixel
        float4 curr_px = output_packed_total_radiance;

        float total_sample_count = curr_px.w + prev_px.w;
        // the later the sampling, the less the contribution is
        float sample_weight = curr_px.w / max(1.0, total_sample_count);

        curr_px.rgb /= max(1.0, curr_px.w);

        output_tex[px] = float4(max(0.0.xxx, lerp(prev_px.rgb, curr_px.rgb, sample_weight)), max(1, total_sample_count));
    }
}