#ifndef _PACK_UNPACK_HLSL_
#define _PACK_UNPACK_HLSL_

float unpack_unorm(uint packed, uint bitCount) {
	uint maxVal = (1u << bitCount) - 1;
	return float(packed & maxVal) / maxVal;
}

uint pack_unorm(float val, uint bitCount) {
	uint maxVal = (1u << bitCount) - 1;
	return uint(clamp(val, 0.0, 1.0) * maxVal);
}

float pack_normal_11_10_11(float3 normal) {
	uint pckd = 0;
    // shift from [-1, 1] to [0, 1]
	pckd += pack_unorm(normal.x * 0.5 + 0.5, 11);
	pckd += pack_unorm(normal.y * 0.5 + 0.5, 10) << 11;
	pckd += pack_unorm(normal.z * 0.5 + 0.5, 11) << 21;
	return asfloat(pckd);
}

float3 unpack_normal_11_10_11(float packed) {
	uint p = asuint(packed);
    // shift from [0, 1] back to [-1, 1]
	return normalize(float3(
		unpack_unorm(p, 11),
		unpack_unorm(p >> 11, 10),
		unpack_unorm(p >> 21, 11)
	) * 2.0 - 1.0);
}

uint pack_color_888_uint(float3 color) {
    // why here we need to sqrt the value?
    color = sqrt(color);
	uint packed = 0;
	packed += pack_unorm(color.r, 8);
	packed += pack_unorm(color.g, 8) << 8;
	packed += pack_unorm(color.b, 8) << 16;
    return packed;
}

float3 unpack_color_888_uint(uint packed) {
    float3 color = 0;
    color.r = unpack_unorm(packed, 8);
    color.g = unpack_unorm(packed >> 8, 8);
    color.b = unpack_unorm(packed >> 16, 8);
    return color * color;
}

uint pack_2x16f_uint(float2 value) {
    uint res = 0;
    res |= f32tof16(value.x);
    res |= f32tof16(value.y) << 16;
    return res;
}

float2 unpack_2x16f_uint(uint value) {
    float2 res;
    res.x = asfloat(value & 0xffff);
    res.y = asfloat((value >> 16) & 0xffff);
    return res;
}

#endif