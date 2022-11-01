use std::collections::HashMap;

use ash::vk;

use super::Device;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct SamplerDesc {
    pub filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_mode: vk::SamplerAddressMode,
}

impl Device {
    pub fn get_immutable_sampler(&self, desc: SamplerDesc) -> vk::Sampler {
        *self.immutable_samplers.get(&desc)
            .unwrap_or_else(|| panic!("Failed to get sampler with {:?}", desc))
    }

    pub(crate) fn create_immutable_samplers(device: &ash::Device) -> HashMap<SamplerDesc, vk::Sampler> {
        // create all combinations
        let filters = [vk::Filter::LINEAR, vk::Filter::NEAREST];
        let mipmap_modes = [vk::SamplerMipmapMode::LINEAR, vk::SamplerMipmapMode::NEAREST];      
        let address_modes = [
            vk::SamplerAddressMode::CLAMP_TO_BORDER, vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::REPEAT, vk::SamplerAddressMode::MIRRORED_REPEAT, vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE,
        ];

        let mut map = HashMap::new();

        for filter in filters {
            for mipmap_mode in mipmap_modes {
                for address_mode in address_modes {
                    let anisotropy_enable = filter == vk::Filter::LINEAR;

                    let sampler = unsafe {
                        device.create_sampler(
                &vk::SamplerCreateInfo::builder()
                                .min_filter(filter)
                                .mag_filter(filter)
                                .mipmap_mode(mipmap_mode)
                                .address_mode_u(address_mode)
                                .address_mode_v(address_mode)
                                .address_mode_w(address_mode)
                                .max_lod(vk::LOD_CLAMP_NONE)
                                .max_anisotropy(16.0)
                                .anisotropy_enable(anisotropy_enable)
                                .build(),
                        None
                        )
                        .expect("Failed to create device immutable sampler!")
                    };

                    map.insert(SamplerDesc { filter, mipmap_mode, address_mode }, sampler);
                }
            }
        }

        map
    }
}