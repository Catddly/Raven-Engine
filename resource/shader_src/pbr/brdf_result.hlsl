#ifndef _BRDF_RESULT_HLSL_
#define _BRDF_RESULT_HLSL_

struct BrdfResult
{
    // the evaluated value
    float3 value;
    float  pdf;
    float3 value_over_pdf;
    float3 transmission_ratio;

    static BrdfResult invalid()
    {
        BrdfResult result;
        result.value = 0.0.xxx;
        result.transmission_ratio = 0.0.xxx;
        result.pdf = 0.0;
        result.value_over_pdf = 0.0.xxx;
        return result;
    }
};

struct BrdfSample : BrdfResult
{
    float3 wi;
    // actually, weight is just the value_over_pdf in BrdfResult.
    // They are the same.
    //float3 weight;

    static BrdfSample invalid()
    {
        BrdfSample result;
        result.value = 0.0.xxx;
        result.pdf = 0.0;
        result.value_over_pdf = 0.0.xxx;
        result.transmission_ratio = 0.0.xxx;
        result.wi = float3(0.0, 0.0, -1.0);
        return result;
    }

    // perform sample on tangent space
    // is wi.z is negative, means this vector is on the opposite direction of the hemisphere
    bool is_valid()
    {
        return wi.z > 1e-6;
    }
};


#endif