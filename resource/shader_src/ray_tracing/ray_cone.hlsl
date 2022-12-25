#ifndef _RAY_CONE_HLSL_
#define _RAY_CONE_HLSL_

#include "../common/bindless_resources.hlsl"
#include "../common/frame_constants.hlsl"
#include "../common/texture.hlsl"

// Paper[1] https://media.contentapi.ea.com/content/dam/ea/seed/presentations/2019-ray-tracing-gems-chapter-20-akenine-moller-et-al.pdf 
// this paper describes a method using ray cone tracing to approximate the Lod used in texture filtering.

struct RayCone {
    // the ray cone width at a specific time
    float width;        // Wi in the paper
    // the ray cone spread angle in radians
    float spread_angle; // Yi in the paper

    // equation 30. in Paper[1]
    // frame_constants.view_constants.clip_to_view._11 == tan(vertical_field_of_view / 2.0)
    static float pixel_spread_angle_from_image_height(float img_height)
    {
        return atan(2.0 * frame_constants_dyn.camera_matrices.clip_to_view._11 / img_height);
    }

    // figure 3. and equation 29. in Paper[1]
    // Propagate ray cone width based on the hit point distance.
    RayCone propagate(float hit_t, float subsequent_spread_angle)
    {
        RayCone new_cone;
        new_cone.width = this.spread_angle * hit_t + this.width;
        new_cone.spread_angle = this.spread_angle + subsequent_spread_angle;
        return new_cone;
    }

    float cone_width_at(float distance)
    {
        return this.width + this.spread_angle * distance; 
    }

    static RayCone from_image_height(float height)
    {
        RayCone res;
        res.width = 0.0;
        res.spread_angle = pixel_spread_angle_from_image_height(height);
        return res;
    }

    static RayCone from_spread_angle(float angle_in_radians)
    {
        RayCone res;
        res.width = 0.0;
        res.spread_angle = angle_in_radians;
        return res;
    }
};

// equation 34. in Paper[1]
BindlessTextureWithLod ray_cone_sample_texture_with_lod(
    uint bindless_tex_id, float delta_constants,
    float cone_width, float3 geometric_normal, float3 ray_direction
)
{
    const float2 wh = bindless_texture_sizes[bindless_tex_id].xy;

    float lambda = delta_constants;
    // defer compute the texture width and height here, not in the delta_constants,
    lambda += 0.5 * log2(wh.x * wh.y);
    lambda += log2(abs(cone_width));
    lambda -= log2(abs(dot(geometric_normal, normalize(ray_direction))));

    BindlessTextureWithLod res;
    res.texture = bindless_textures[NonUniformResourceIndex(bindless_tex_id)];
    res.lod = lambda;
    return res;
}


#endif