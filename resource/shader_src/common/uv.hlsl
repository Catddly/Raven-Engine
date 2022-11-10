#ifndef _UV_HLSL_
#define _UV_HLSL_

float2 pixel_to_uv(float2 pixel, float2 resolution)
{
    // shift 0.5 here, because we want to use this uv to calculate the NDC coordinates of some pixel.
    return (pixel + 0.5.xx) / resolution;
}

// vulkan only
float2 uv_to_clip(float2 uv)
{
    return (uv - 0.5.xx) * float2(2.0, -2.0);
}

// Convert a cubemap texel coordinate to the cubemap sample direction.
float3 cube_to_world_dir(int3 coord, float2 cubemap_size)
{
    float2 tex_coord = float2(coord.xy) / cubemap_size;
    tex_coord = tex_coord * 2.0 - 1.0; 

    switch (coord.z)
    {
        case 0: return float3(1.0, -tex_coord.yx);              // +X
        case 1: return float3(-1.0, -tex_coord.y, tex_coord.x); // -X
        case 2: return float3(tex_coord.x, 1.0, tex_coord.y);   // +Y
        case 3: return float3(tex_coord.x, -1.0, -tex_coord.y); // -Y
        case 4: return float3(tex_coord.x, -tex_coord.y, 1.0);  // +Z
        case 5: return float3(-tex_coord.xy, -1.0);             // -Z
    }
    return 0.0.xxx;
}

// Convert a cubemap texture coordinate to the exact texel cell coordinate.
int3 tex_coord_to_cube(float3 tex_coord, float2 cubemap_size)
{
    float3 abst = abs(tex_coord);
    tex_coord /= max(max(abst.x, abst.y), abst.z);

    float cubeFace;
    float2 uvCoord;
    if (abst.x > abst.y && abst.x > abst.z) 
    {
        // x major
        float negx = step(tex_coord.x, 0.0);
        uvCoord = lerp(-tex_coord.zy, float2(tex_coord.z, -tex_coord.y), negx);
        cubeFace = negx;
    } 
    else if (abst.y > abst.z) 
    {
        // y major
        float negy = step(tex_coord.y, 0.0);
        uvCoord = lerp(tex_coord.xz, float2(tex_coord.x, -tex_coord.z), negy);
        cubeFace = 2.0 + negy;
    } 
    else 
    {
        // z major
        float negz = step(tex_coord.z, 0.0);
        uvCoord = lerp(float2(tex_coord.x, -tex_coord.y), -tex_coord.xy, negz);
        cubeFace = 4.0 + negz;
    }

    uvCoord = (uvCoord + 1.0) * 0.5;
    uvCoord = uvCoord * cubemap_size;
    uvCoord = clamp(uvCoord, 0.0.xx, cubemap_size - 1.0.xx);

    return int3(int2(uvCoord), int(cubeFace));
}

#endif