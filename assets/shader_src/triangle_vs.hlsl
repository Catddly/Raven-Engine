struct VSOutput {
    float4 pos: SV_POSITION;
    [[vk::location(0)]] float3 color: COLOR0;
};

VSOutput main(uint vid: SV_VertexID)
{
    const float2 Vertices[3] = {
        float2( 0.0,  1.0),
        float2(-1.0, -1.0),
        float2( 1.0, -1.0),
    };

    const float3 Colors[3] = {
        float3(0.0, 1.0, 0.0),
        float3(1.0, 0.0, 0.0),
        float3(0.0, 0.0, 1.0)
    };

    VSOutput output = (VSOutput)0;
    output.pos = float4(Vertices[vid], 0.0, 1.0);
    output.color = Colors[vid];
    return output;
}