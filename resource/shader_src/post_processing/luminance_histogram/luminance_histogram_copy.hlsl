[[vk::binding(0)]] StructuredBuffer<uint>   src_histogram;
[[vk::binding(1)]] RWStructuredBuffer<uint> dst_histogram;

[numthreads(256, 1, 1)]
void main(uint idx: SV_DispatchThreadID)
{
    dst_histogram[idx] = src_histogram[idx];
}
