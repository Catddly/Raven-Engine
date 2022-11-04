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
[[vk::binding(1)]] RWTexture2DArray<float4> prefilter_cube_map;

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

float3 uniform_sample_cone(float2 urand, float cos_theta_max) {
    float cos_theta = (1.0 - urand.x) + urand.x * cos_theta_max;
    float sin_theta = sqrt(saturate(1.0 - cos_theta * cos_theta));
    float phi = urand.y * 6.28318530717958647692528676655900577;
    return float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta);
}

// float3 ImportanceSampleGGX(float2 Xi, float3 N, float roughness)
// {
//     float a = roughness*roughness;
	
//     float phi = 2.0 * PI * Xi.x;
//     float cosTheta = sqrt((1.0 - Xi.y) / (1.0 + (a*a - 1.0) * Xi.y));
//     float sinTheta = sqrt(1.0 - cosTheta*cosTheta);
	
//     // from spherical coordinates to cartesian coordinates
//     float3 H;
//     H.x = cos(phi) * sinTheta;
//     H.y = sin(phi) * sinTheta;
//     H.z = cosTheta;
	
//     // from tangent-space vector to world-space sample vector
//     float3 up        = abs(N.z) < 0.999 ? float3(0.0, 0.0, 1.0) : float3(1.0, 0.0, 0.0);
//     float3 tangent   = normalize(cross(up, N));
//     float3 bitangent = cross(N, tangent);
	
//     float3 sampleVec = tangent * H.x + bitangent * H.y + N * H.z;
//     return normalize(sampleVec);
// }  

[numthreads(8, 8, 1)]
void main(in uint3 px: SV_DispatchThreadID) {
    const float3x3 CUBE_MAP_FACE_ROTATIONS[6] = {
        float3x3( 0,  0, -1, 
                  0, -1,  0, 
                 -1,  0,  0),   // right
        float3x3(0,0,1, 
                 0,-1,0, 
                 1,0,0),     // left

        float3x3(1,0,0, 
                 0,0,-1, 
                 0,1,0),     // top
        float3x3(1,0,0, 
                 0,0,1, 
                 0,-1,0),     // bottom

        float3x3(1,0,0, 
                 0,-1,0, 
                 0,0,-1),    // back
        float3x3(-1,0,0, 
                  0,-1,0, 
                  0,0,1),    // front
    };

    uint face = px.z;
    float2 uv = (px.xy + 0.5) / push_constants.render_res_width;

    float3 output_dir = normalize(mul(CUBE_MAP_FACE_ROTATIONS[face], float3(uv * 2 - 1, -1.0)));
    const float3x3 clip_to_world = build_orthonormal_basis(output_dir);

    static const uint sample_count = 512;

    float4 result = 0;
    for (uint i = 0; i < sample_count; ++i) {
        float2 urand = hammersley(i, sample_count);
        float3 input_dir = mul(clip_to_world, uniform_sample_cone(urand, 0.99));
        result += cube_map.SampleLevel(sampler_llr, input_dir, 0);
    }

    prefilter_cube_map[px] = result / sample_count;

    // uint face = px.z;
    // float2 uv = pixel_to_uv(px.xy, float2(push_constants.render_res_width, push_constants.render_res_height));

    // float3 output_dir = normalize(mul(CUBE_MAP_FACE_ROTATIONS[face], float3(uv * 2 - 1, -1.0)));
    // const float3x3 orthonormal_basis = build_orthonormal_basis(output_dir);

    // float3 N = normalize(mul(orthonormal_basis, output_dir));
    // float3 R = N;
    // float3 V = R;

    // const uint SAMPLE_COUNT = 1024;
    // float totalWeight = 0.0;   
    // float3 prefiltered_color = 0.0.xxx;    

    // for(uint i = 0; i < SAMPLE_COUNT; ++i)
    // {
    //     float2 Xi = Hammersley(i, SAMPLE_COUNT);
    //     float3 H  = ImportanceSampleGGX(Xi, N, 0.0);
    //     float3 L  = normalize(2.0 * dot(V, H) * H - V);

    //     float NdotL = max(dot(N, L), 0.0);
    //     if(NdotL > 0.0)
    //     {
    //         prefiltered_color += cube_map.SampleLevel(sampler_llce, L, 0).rgb * NdotL;
    //         totalWeight       += NdotL;
    //     }
    // }
    // prefiltered_color = prefiltered_color / totalWeight;

    // prefilter_cube_map[px] = float4(prefiltered_color, 1.0);
}