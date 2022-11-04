#ifndef _ROUGHNESS_ADJUST_HLSL_
#define _ROUGHNESS_ADJUST_HLSL_

// See https://catlikecoding.com/unity/tutorials/scriptable-render-pipeline/reflections/
float roughness_to_perceptual_roughness(float roughness) {
    return sqrt(roughness);
}

float perceptual_roughness_to_roughness(float percep_roughness) {
    return percep_roughness * percep_roughness;
}

#endif