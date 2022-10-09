use std::hash;

use turbosloth::*;
use bytes::Bytes;

use super::asset_manager::AssetHandle;
use super::error::AssetPipelineError;
use super::{RawAsset, Texture, AssetType, Mesh, PackedVertex, Material, TextureSource};

/// Consume a raw asset and turn it into a AssetHandle which reference a storage asset.
pub struct AssetProcessor {
    raw_asset: Box<dyn RawAsset>,
}

impl AssetProcessor {
    pub fn new(raw_asset: Box<dyn RawAsset>) -> Self {
        Self {
            raw_asset,
        }
    }

    pub fn process(self) -> anyhow::Result<Lazy<AssetHandle>, AssetPipelineError> {
        let ty = self.raw_asset.asset_type();
        let asset = match ty {
            AssetType::Mesh => {
                let raw_mesh = self.raw_asset.as_mesh().ok_or(AssetPipelineError::ProcessFailure)?.clone();
                RawMeshProcess::new(raw_mesh).into_lazy()
            }
            _ => unimplemented!(),
        };

        Ok(asset)
    }
}

// TODO: merge process work together
#[derive(Clone)]
struct RawMeshProcess {
    raw: Mesh::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawMeshProcess {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawMeshProcess {
    pub fn new(raw: Mesh::Raw) -> Self {
        let asset_registry = super::asset_manager::get_runtime_asset_registry();
        let handle = asset_registry.write().register_empty_asset();

        Self {
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

    async fn run(mut self, _cx: RunContext) -> Self::Output {
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
            .map(|raw| RawTextureProcess::new(raw).into_lazy())
            .collect::<Vec<_>>();

        let materials = self.raw.materials.into_iter()
            .map(|raw| RawMaterialProcess::new(raw).into_lazy())
            .collect::<Vec<_>>();

        let storage = Mesh::Storage {
            packed: packed_vertex,
            colors: self.raw.colors,
            tangents: self.raw.tangents,
            uvs: self.raw.uvs,
            indices: self.raw.indices,

            materials: materials,
            material_textures: textures,
            material_ids: self.raw.material_ids,
        };

        let asset_registry = super::asset_manager::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, Box::new(storage));
    
        Ok(self.handle)
    }
}

#[derive(Clone)]
struct RawTextureProcess {
    raw: Texture::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawTextureProcess {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawTextureProcess {
    pub fn new(raw: Texture::Raw) -> Self {
        let asset_registry = super::asset_manager::get_runtime_asset_registry();
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
            },
            // TODO: implementing byte interpret
            TextureSource::Bytes(_) => unimplemented!(),
            TextureSource::Source(_) => unimplemented!(),
        };

        let storage = Texture::Storage {
            extent: [1, 1, 1], 
            lod_groups: vec![bytes.to_vec()],
        };

        let asset_registry = super::asset_manager::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, Box::new(storage));

        Ok(self.handle)
    }
}

#[derive(Clone)]
struct RawMaterialProcess {
    raw: Material::Raw,
    handle: AssetHandle,
}

impl std::hash::Hash for RawMaterialProcess {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.handle)
    }
}

impl RawMaterialProcess {
    pub fn new(raw: Material::Raw) -> Self {
        let asset_registry = super::asset_manager::get_runtime_asset_registry();
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
        let storage = Material::Storage {
            metallic: self.raw.metallic,
            roughness: self.raw.roughness,
            base_color: self.raw.base_color,
            emissive: self.raw.emissive,
            texture_mapping: self.raw.texture_mapping,
            texture_transform: self.raw.texture_transform,
        };

        let asset_registry = super::asset_manager::get_runtime_asset_registry();
        asset_registry.write().update_asset(&mut self.handle, Box::new(storage));

        Ok(self.handle)
    }
}