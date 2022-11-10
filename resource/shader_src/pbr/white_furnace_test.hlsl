#ifndef _WHITE_FURNACE_TEST_HLSL_
#define _WHITE_FURNACE_TEST_HLSL_

#include "../math/math.hlsl"

#include "../pbr/brdf.hlsl"

uint reverse_bits(uint v)
{
    v = ((v >> 1) & 0x55555555) | ((v & 0x55555555) << 1);
	v = ((v >> 2) & 0x33333333) | ((v & 0x33333333) << 2);
	v = ((v >> 4) & 0x0F0F0F0F) | ((v & 0x0F0F0F0F) << 4);
	v = ((v >> 8) & 0x00FF00FF) | ((v & 0x00FF00FF) << 8);
	v = (v >> 16) | (v << 16);
	return v;
}

// TODO: my implementation has something wrong. 
float3 white_furnace_test(float roughness, float ndotv, float3 mult)
{
    const float alpha = roughness * roughness;
	const float alpha2 = alpha * alpha;

    float3 wo = float3(sqrt(1.0f - ndotv * ndotv), 0.0, ndotv);
    const float3 normal = float3(0.0, 0.0, 1.0);

	float3 integral = 0.0.xxx;
	uint num_samples = 4096;

	for (uint i = 0; i < num_samples; ++i)
	{
		float e1 = (float)i / num_samples;
		float e2 = (float)((float)reverse_bits(i) / (float)0x100000000);

		float phi = 2.0 * PI * e1;
		float cosTheta = sqrt((1.0f - e2) / (1.0f + (alpha2 - 1.0f) * e2)); // importance_sample_ggx
		float sinTheta = sqrt(1.0f - cosTheta * cosTheta);
	
        // From spherical coordinates to cartesian coordinates
        float3 H = float3(cos(phi) * sinTheta, sin(phi) * sinTheta, cosTheta);

        float3 wi = reflect(-wo, H);

		float ndotl = max(dot(normal, wi), 0.0);
		float ndoth = max(dot(normal, H), 0.0);
		float vdoth = max(dot(wo, H), 0.0);

        const float  d = SpecularBrdf::ndf_ggx(max(0.00001, alpha2), ndoth);
		const float  g = ShadowMaskTermSmith::smith_ggx_height_correlated(ndotv, ndotl, alpha2);
        const float3 f = fresnel_schlick(0.04, 1.0, 0.0);

		integral += (d * g * wi.z);
	}

	integral = integral / num_samples;
	return integral;
}

#endif