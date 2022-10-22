#ifndef _FRAME_CONSTANTS_HLSL_
#define _FRAME_CONSTANTS_HLSL_

struct CameraMatrices {
    float4x4 world_to_view;
    float4x4 view_to_world;
    float4x4 view_to_clip;
    float4x4 clip_to_view;
};

// Same in raven-rg::executor::DrawFrameContext
struct FrameConstants {
    CameraMatrices camera_matrices;
};

[[vk::binding(0, 2)]] ConstantBuffer<FrameConstants> frame_constants_dyn;

#endif