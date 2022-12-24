#ifndef _TEXTURE_HLSL_
#define _TEXTURE_HLSL_

struct BindlessTextureWithLod
{
    Texture2D texture;
    float     lod;

    float4 sample_tex(SamplerState samplerr, float2 uv)
    {
        return this.texture.SampleLevel(samplerr, uv, this.lod);
    }
};

#endif