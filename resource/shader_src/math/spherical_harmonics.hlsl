#ifndef _SPHERICAL_HARMONICS_HLSL_
#define _SPHERICAL_HARMONICS_HLSL_

#include "../math/constants.hlsl"

struct SphericalHarmonicsBasis
{
    float Y00;
    float Y1_1;
    float Y10;
    float Y11;
    float Y2_2;
    float Y2_1;
    float Y20;
    float Y21;
    float Y22;

    static SphericalHarmonicsBasis zero()
    {
        SphericalHarmonicsBasis basis;
        basis.Y00  = 0.0;
        basis.Y1_1 = 0.0;
        basis.Y10  = 0.0;
        basis.Y11  = 0.0;
        basis.Y2_2 = 0.0;
        basis.Y2_1 = 0.0;
        basis.Y20  = 0.0;
        basis.Y21  = 0.0;
        basis.Y22  = 0.0;
        return basis;
    }

    static SphericalHarmonicsBasis get_coefficients()
    {
        SphericalHarmonicsBasis basis;
        basis.Y00  = 0.282095;
        basis.Y1_1 = 0.488603;
        basis.Y10  = 0.488603;
        basis.Y11  = 0.488603;
        basis.Y2_2 = 1.092548;
        basis.Y2_1 = 1.092548;
        basis.Y20  = 0.315392;
        basis.Y21  = 1.092548;
        basis.Y22  = 0.546274;
        return basis;
    }

    SphericalHarmonicsBasis mul_basis(SphericalHarmonicsBasis basis)
    {
        SphericalHarmonicsBasis res;
        res.Y00  = Y00  * basis.Y00;
        res.Y1_1 = Y1_1 * basis.Y1_1;
        res.Y10  = Y10  * basis.Y10;
        res.Y11  = Y11  * basis.Y11;
        res.Y2_2 = Y2_2 * basis.Y2_2;
        res.Y2_1 = Y2_1 * basis.Y2_1;
        res.Y20  = Y20  * basis.Y20;
        res.Y21  = Y21  * basis.Y21;
        res.Y22  = Y22  * basis.Y22;
        return res;
    }

    static SphericalHarmonicsBasis from_direction(float3 dir)
    {
        const float x = dir.x;
        const float y = dir.y;
        const float z = dir.z;
        const float x2 = x * x;
        const float y2 = y * y;
        const float z2 = z * z;

        SphericalHarmonicsBasis basis;
        basis.Y00  = 1.0;
        basis.Y1_1 = y;
        basis.Y10  = z;
        basis.Y11  = x;
        basis.Y2_2 = x * y;
        basis.Y2_1 = y * z;
        basis.Y20  = 3.0 * z2 - 1.0;
        basis.Y21  = x * z;
        basis.Y22  = x2 - y2;

        return basis.mul_basis(get_coefficients());
    }

    SphericalHarmonicsBasis mul_scaler(float scaler)
    {
        SphericalHarmonicsBasis res;
        res.Y00  = Y00  * scaler;
        res.Y1_1 = Y1_1 * scaler;
        res.Y10  = Y10  * scaler;
        res.Y11  = Y11  * scaler;
        res.Y2_2 = Y2_2 * scaler;
        res.Y2_1 = Y2_1 * scaler;
        res.Y20  = Y20  * scaler;
        res.Y21  = Y21  * scaler;
        res.Y22  = Y22  * scaler;
        return res;
    }

    float dot(SphericalHarmonicsBasis basis)
    {
        float value = 0.0;
        value += Y00  * basis.Y00;
        value += Y1_1 * basis.Y1_1;
        value += Y10  * basis.Y10;
        value += Y11  * basis.Y11;
        value += Y2_2 * basis.Y2_2;
        value += Y2_1 * basis.Y2_1;
        value += Y20  * basis.Y20;
        value += Y21  * basis.Y21;
        value += Y22  * basis.Y22;
        return value;
    }

    SphericalHarmonicsBasis add(SphericalHarmonicsBasis basis)
    {
        SphericalHarmonicsBasis res;
        res.Y00  = Y00  + basis.Y00;
        res.Y1_1 = Y1_1 + basis.Y1_1;
        res.Y10  = Y10  + basis.Y10;
        res.Y11  = Y11  + basis.Y11;
        res.Y2_2 = Y2_2 + basis.Y2_2;
        res.Y2_1 = Y2_1 + basis.Y2_1;
        res.Y20  = Y20  + basis.Y20;
        res.Y21  = Y21  + basis.Y21;
        res.Y22  = Y22  + basis.Y22;
        return res;
    }
};

void store_sh(out float coeffs[9], in SphericalHarmonicsBasis basis)
{
    coeffs[0] = basis.Y00;
    coeffs[1] = basis.Y1_1;
    coeffs[2] = basis.Y10;
    coeffs[3] = basis.Y11;
    coeffs[4] = basis.Y2_2;
    coeffs[5] = basis.Y2_1;
    coeffs[6] = basis.Y20;
    coeffs[7] = basis.Y21;
    coeffs[8] = basis.Y22;
}

SphericalHarmonicsBasis load_sh(in float coeffs[9])
{
    SphericalHarmonicsBasis basis;
    basis.Y00  = coeffs[0];
    basis.Y1_1 = coeffs[1];
    basis.Y10  = coeffs[2];
    basis.Y11  = coeffs[3];
    basis.Y2_2 = coeffs[4];
    basis.Y2_1 = coeffs[5];
    basis.Y20  = coeffs[6];
    basis.Y21  = coeffs[7];
    basis.Y22  = coeffs[8];
    return basis;
}

#endif