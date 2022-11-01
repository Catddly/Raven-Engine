use std::hash::Hasher;
use std::path::PathBuf;
use std::marker::PhantomData;

use image::DynamicImage;
use image::imageops::FilterType;
use turbosloth::*;
use bytes::Bytes;
use wyhash::WyHash;

use crate::math;

use super::asset_registry::{AssetHandle, AssetRef};
use super::error::AssetPipelineError;
use super::{RawAsset, Texture, AssetType, Mesh, PackedVertex, Material, TextureSource};

/// Consume a raw asset and turn it into a AssetHandle which reference a storage asset.
pub struct AssetProcessor {
    uri: PathBuf,
    raw_asset: Box<dyn RawAsset>,
}

impl AssetProcessor {
    pub fn new(uri: impl Into<PathBuf>, raw_asset: Box<dyn RawAsset>) -> Self {
        Self {
            uri: uri.into(),
            raw_asset,
        }
    }

    pub fn process(self) -> anyhow::Result<Lazy<AssetHandle>, AssetPipelineError> {
        let ty = self.raw_asset.asset_type();
        let asset = match ty {
            AssetType::Mesh => {
                let raw_mesh = self.raw_asset.as_mesh().ok_or(AssetPipelineError::ProcessFailure)?.clone();
                RawMeshProcess::new(self.uri, raw_mesh).into_lazy()
            }
            AssetType::Texture => {
                let raw_tex = self.raw_asset.as_texture().ok_or(AssetPipelineError::ProcessFailure)?.clone();
                RawTextureProcess::new(raw_tex).into_lazy()
            }
            _ => unimplemented!(),
        };

        Ok(asset)
    }
}

fn calc_asset_uuid(base_path: &PathBuf, sub_dependent_index: usize) -> u64 {
    assert!(base_path.is_relative() && !base_path.is_dir());

    // the seed of root is 0.
    let mut hasher = WyHash::with_seed(sub_dependent_index as u64);
    let string_lossy = base_path.to_string_lossy();
    let bytes = unsafe { std::slice::from_raw_parts(string_lossy.as_ptr(), string_lossy.len()) };
    hasher.write(bytes);

    hasher.finish()
}

// TODO: merge process work together
#[derive(Clone)]
struct RawMeshProcess {
    uri: PathBuf,
    raw: Mesh::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawMeshProcess {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawMeshProcess {
    pub fn new(uri: PathBuf, raw: Mesh::Raw) -> Self {
        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        let handle = asset_registry.write().register_empty_asset();

        Self {
            uri,
            raw,
            handle,
        }
    }
}

impl RawMeshProcess {
    fn pack_unit_direction_11_10_11(x: f32, y: f32, z: f32) -> u32 {
        let x = ((x.max(-1.0).min(1.0) * 0.5 + 0.5) * ((1u32 << 11u32) - 1u32) as f32) as u32;
        let y = ((y.max(-1.0).min(1.0) * 0.5 + 0.5) * ((1u32 << 10u32) - 1u32) as f32) as u32;
        let z = ((z.max(-1.0).min(1.0) * 0.5 + 0.5) * ((1u32 << 11u32) - 1u32) as f32) as u32;
    
        (z << 21) | (y << 11) | x
    }
}

#[async_trait]
impl LazyWorker for RawMeshProcess {
    type Output = anyhow::Result<AssetHandle>;

    async fn run(mut self, ctx: RunContext) -> Self::Output {
        // vertex packing
        let mut packed_vertex = Vec::with_capacity(self.raw.positions.len());

        for (idx, pos) in self.raw.positions.iter().enumerate() {
            let [nx, ny, nz] = self.raw.normals[idx];

            packed_vertex.push(PackedVertex {
                position: *pos,
                normal: Self::pack_unit_direction_11_10_11(nx, ny, nz),
            });
        }

        // process mesh's raw materials and textures
        let textures = self.raw.material_textures.into_iter()
            .map(|raw| RawTextureProcess::new(raw).into_lazy().eval(&ctx))
            .collect::<Vec<_>>();

        let materials = self.raw.materials.into_iter()
            .map(|raw| RawMaterialProcess::new(raw).into_lazy().eval(&ctx))
            .collect::<Vec<_>>();

        let mut resource_dependent_index = 0;
        let materials = smol::block_on(futures::future::try_join_all(materials.into_iter()))?
            .into_iter()
            .map(|handle| {
                resource_dependent_index += 1;

                AssetRef {
                    handle,
                    uuid: calc_asset_uuid(&self.uri, resource_dependent_index),
                    _marker: PhantomData,
                }
            }
            )
            .collect::<Vec<AssetRef<Material::Storage>>>();

        let textures = smol::block_on(futures::future::try_join_all(textures.into_iter()))?
            .into_iter()
            .map(|handle| {
                resource_dependent_index += 1;

                AssetRef {
                    handle,
                    uuid: calc_asset_uuid(&self.uri, resource_dependent_index),
                    _marker: PhantomData,
                }
            }
            )
            .collect::<Vec<AssetRef<Texture::Storage>>>();

        let storage = Box::new(Mesh::Storage {
            packed: packed_vertex,
            colors: self.raw.colors,
            tangents: self.raw.tangents,
            uvs: self.raw.uvs,
            indices: self.raw.indices,

            materials: materials,
            material_textures: textures,
            material_ids: self.raw.material_ids,
        });

        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, storage);
    
        Ok(self.handle)
    }
}

#[derive(Clone)]
struct RawTextureProcess {
    raw: Texture::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawTextureProcess {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawTextureProcess {
    pub fn new(raw: Texture::Raw) -> Self {
        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        let handle = asset_registry.write().register_empty_asset();

        Self {
            raw,
            handle
        }
    }
}

#[async_trait]
impl LazyWorker for RawTextureProcess {
    type Output = anyhow::Result<AssetHandle>;

    async fn run(mut self, _cx: RunContext) -> Self::Output {
        let bytes = match self.raw.source {
            TextureSource::Empty => unreachable!(),
            TextureSource::Placeholder(pc) => {
                Bytes::copy_from_slice(&pc)
            }
            TextureSource::Bytes(bytes) => {
                bytes
            },
        };

        let desc = &self.raw.desc;

        // TODO: other color bits
        let image = image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(
                desc.extent[0], 
                desc.extent[1], 
                bytes.to_vec()
            )
            .unwrap()
        );

        let down_sample = |image: &DynamicImage| {
            let width = image.width() >> 1;
            let height = image.height() >> 1;

            image.resize_exact(width, height, FilterType::Lanczos3)
        };

        // generate mipmap bytes
        let lod_groups = if desc.use_mipmap {
            let mipmap_level = math::max_mipmap_level(desc.extent[0], desc.extent[1]);

            let mut mips = Vec::new();
            // level 0
            let mut image = {
                let mip = down_sample(&image);
                mips.push(image.into_rgba8().into_raw());
                mip
            };

            for _ in 1..mipmap_level {
                let next = down_sample(&image);
                let mip = std::mem::replace(&mut image, next);
                mips.push(mip.into_rgba8().into_raw());
            }

            mips
        } else {
            vec![image.into_rgba8().into_raw()]
        };

        let storage = Box::new(Texture::Storage {
            extent: desc.extent, 
            lod_groups,
        });

        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, storage);

        Ok(self.handle)
    }
}

#[derive(Clone)]
struct RawMaterialProcess {
    raw: Material::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawMaterialProcess {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawMaterialProcess {
    pub fn new(raw: Material::Raw) -> Self {
        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        let handle = asset_registry.write().register_empty_asset();

        Self {
            raw,
            handle,
        }
    }
}

#[async_trait]
impl LazyWorker for RawMaterialProcess {
    type Output = anyhow::Result<AssetHandle>;

    async fn run(mut self, _cx: RunContext) -> Self::Output {
        let storage = Box::new(Material::Storage {
            metallic: self.raw.metallic,
            roughness: self.raw.roughness,
            base_color: self.raw.base_color,
            emissive: self.raw.emissive,
            texture_mapping: self.raw.texture_mapping,
            texture_transform: self.raw.texture_transform,
        });

        let asset_registry = super::asset_registry::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, storage);

        Ok(self.handle)
    }
}