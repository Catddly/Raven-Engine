#ifndef _CAMERA_RAY_HLSL_
#define _CAMERA_RAY_HLSL_

#include "../common/uv.hlsl"
#include "../common/frame_constants.hlsl"

struct CameraRayContext
{
    float2 cs_coord;

    float4 origin_cs;
    float4 direction_cs;
    float4 origin_vs;
    float4 direction_vs;
    float4 origin_ws;
    float4 direction_ws;

    float3 get_position_ws()
    {
        return origin_ws.xyz / origin_ws.w;
    }

    float3 get_direction_ws()
    {
        return normalize(direction_ws.xyz);
    }

    float3 get_frag_position_ws(float hit_depth)
    {
        const float4 hit_pos_cs = float4(this.cs_coord, hit_depth, 1.0);

        CameraFrameConstants cam = frame_constants_dyn.camera_constants;

        const float4 hit_pos_vs = mul(cam.clip_to_view, hit_pos_cs);
        const float4 hit_pos_ws_homo = mul(cam.view_to_world, hit_pos_vs);
        const float3 hit_pos_ws = hit_pos_ws_homo.xyz / hit_pos_ws_homo.w;
        return hit_pos_ws;
    }

    static CameraRayContext from_screen_uv(float2 uv)
    {
        CameraRayContext ctx;

        ctx.cs_coord = uv_to_clip(uv);

        // remember that we reverse z to gain better z precision.
        // so here z = 0.0 is the fartest and z = 1.0 is the nearest.
        ctx.origin_cs = float4(ctx.cs_coord, 1.0, 1.0);
        ctx.direction_cs = float4(ctx.cs_coord, 0.0, 1.0);

        CameraFrameConstants cam = frame_constants_dyn.camera_constants;

        ctx.origin_vs = mul(cam.clip_to_view, ctx.origin_cs);
        ctx.direction_vs = mul(cam.clip_to_view, ctx.direction_cs);

        ctx.origin_ws = mul(cam.view_to_world, ctx.origin_vs);
        ctx.direction_ws = mul(cam.view_to_world, ctx.direction_vs);

        return ctx;
    }
};

#endif