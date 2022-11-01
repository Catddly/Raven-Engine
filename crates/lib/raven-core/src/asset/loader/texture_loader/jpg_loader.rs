use std::io::Read;
use std::path::PathBuf;
use std::fs::File;

use bytes::Bytes;

use crate::asset::{Texture, TextureSource, TextureDesc, TextureGammaSpace, loader};
use crate::asset::loader::{AssetLoader, LoadAssetTextureType};
use crate::filesystem::{self, ProjectFolder};

pub struct JpgTextureLoader {
    path: PathBuf,
    need_gen_mipmap: bool,
}

impl JpgTextureLoader {
    pub fn new(path: PathBuf) -> Self {
        let ty = loader::extract_texture_type(&path).unwrap();
        assert!(matches!(ty, LoadAssetTextureType::Jpg), "Loading jpg resource but found other: {:?}", ty);

        Self {
            path,
            need_gen_mipmap: false,
        }
    }

    pub fn generate_mipmap(mut self, need_gen_mipmap: bool) -> Self {
        self.need_gen_mipmap = need_gen_mipmap;
        self
    }
}

impl AssetLoader for JpgTextureLoader {
    fn load(&self) -> anyhow::Result<Box<dyn crate::asset::RawAsset>> {
        let folder = filesystem::get_project_folder_path_absolute(ProjectFolder::Assets)?;
        let path = folder.join(self.path.clone());
        assert!(path.is_file());
        let mut file = File::open(path)?;
        
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let image = image::load_from_memory(bytes.as_slice())?;
        let width = image.width();
        let height = image.height();

        assert!(width.is_power_of_two());
        assert!(height.is_power_of_two());

        Ok(Box::new(Texture::Raw {
            source: TextureSource::Bytes(Bytes::from(image.into_rgba8().into_raw())),
            desc: TextureDesc {
                extent: [width, height, 1],
                //ty: LoadAssetTextureType::Jpg,
                gamma_space: TextureGammaSpace::Linear,
                use_mipmap: self.need_gen_mipmap,
            },
        }))
    }
}