[[vk::binding(0)]] Texture2D<float4> src_tex;
[[vk::binding(1)]] RWTexture2D<float4> dst_tex;

[numthreads(8, 8, 1)]
void main(in uint2 px: SV_DispatchThreadID) {
    dst_tex[px] = float4(src_tex[px].rgb, 1.0);
}