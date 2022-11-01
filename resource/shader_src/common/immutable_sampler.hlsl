#ifndef _IMMUTABLE_SAMPLERS_HLSL_
#define _IMMUTABLE_SAMPLERS_HLSL_

[[vk::binding(32)]] SamplerState sampler_lnce;
[[vk::binding(33)]] SamplerState sampler_llr;
[[vk::binding(34)]] SamplerState sampler_nnce;
[[vk::binding(35)]] SamplerState sampler_llce;

#endif