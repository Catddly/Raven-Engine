use std::{
    fs,
    io,
    path::PathBuf,
};

use gltf::Gltf;

use crate::{filesystem::{self, ProjectFolder}, asset::loader::loader::LoadAssetMeshType};
use super::super::loader::{self, AssetLoader};

pub struct GltfMeshLoader {
    path: PathBuf,
}

impl GltfMeshLoader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
        }
    }
}

impl AssetLoader for GltfMeshLoader {
    fn load(&self) -> anyhow::Result<()> {
        let mesh_type = loader::extract_mesh_type(&self.path)?;
        assert!(matches!(mesh_type, LoadAssetMeshType::Gltf));

        let dir = filesystem::get_project_folder_path_absolute(ProjectFolder::Assets)?;
        let path = dir.join(self.path.clone());

        let file = fs::File::open(&path)?;
        let reader = io::BufReader::new(file);
        let gltf = Gltf::from_reader(reader)?;

        let meshes = gltf.meshes();

        for mesh in meshes {
            glog::info!("{:?}", mesh.name().unwrap());
        }

        Ok(())
    }
}