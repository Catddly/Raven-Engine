#ifndef _MATERIAL_MODEL_HLSL_
#define _MATERIAL_MODEL_HLSL_

// Remap albedo value in a MR(metallic roughness) meterial model.
float3 MR_model_remap_albedo(float3 albedo, float metallic)
{
    float3 F0 = 0.04.xxx;
    return lerp(F0, albedo, metallic.xxx);
}

#endif