[[vk::binding(0)]] RWStructuredBuffer<uint> output_histogram;

[numthreads(256, 1, 1)]
void main(uint idx: SV_DispatchThreadID)
{
    output_histogram[idx] = 0;
}
