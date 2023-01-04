#ifndef _FRAME_CONSTANTS_HLSL_
#define _FRAME_CONSTANTS_HLSL_

#include "../light/light_commons.hlsl"

struct CameraFrameConstants {
    float4x4 world_to_view;
    float4x4 view_to_world;
    float4x4 view_to_clip;
    float4x4 clip_to_view;
};

struct LightFrameConstants {
    float3 color;     // color in range [0.0, 1.0]
    uint   shadowed;  // it is a bool
    float3 direction; // direction vector
    float  intensity;
};

// Same in raven-rg::executor::DrawFrameContext
struct FrameConstants {
    CameraFrameConstants camera_constants;
    LightFrameConstants  light_constants[MAX_DIRECTIONAL_LIGHT_COUNT];

    uint  frame_index;
    float pre_exposure_mult;
    float pre_exposure_prev_frame_mult;
    float pre_exposure_delta;

    uint  directional_light_count;
    uint  pad0;
    uint  pad1;
    uint  pad2;
};

[[vk::binding(0, 2)]] ConstantBuffer<FrameConstants> frame_constants_dyn;

#endif