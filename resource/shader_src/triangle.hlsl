struct VSOut {
    float4 pos: SV_POSITION;
    [[vk::location(0)]] float3 color: COLOR0;
};

VSOut vs_main(uint vid: SV_VertexID)
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

    VSOut output = (VSOut)0;
    output.pos = float4(Vertices[vid], 0.0, 1.0);
    output.color = Colors[vid];
    return output;
}

struct PSOut {
    float4 color: SV_TARGET;
};

PSOut ps_main([[vk::location(0)]] float3 color: COLOR0)
{
    PSOut psout;
    psout.color = float4(color, 1.0);

    return psout;
}