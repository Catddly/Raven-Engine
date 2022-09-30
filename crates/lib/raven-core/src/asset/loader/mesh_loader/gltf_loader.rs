use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use gltf::{Gltf, Document, buffer::Source as BufferSource, Error, image::Source as ImageSource};
use bytes::Bytes;

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
        assert!(matches!(mesh_type, LoadAssetMeshType::Gltf), "Loading gltf resource but found other: {:?}", mesh_type);

        let dir = filesystem::get_project_folder_path_absolute(ProjectFolder::Assets)?;
        let path = dir.join(self.path.clone());
        assert!(path.is_file(), "Path may not exists or this path is not a file!");

        let file = fs::File::open(&path)?;
        let reader = io::BufReader::new(file);
        let gltf = Gltf::from_reader(reader)?;

        let document = gltf.document;
        let mut blob = gltf.blob;
        let base_path = if let Some(parent) = path.parent() {
            parent.to_owned()
        } else {
            PathBuf::from("./")
        };
        
        let buffers = extract_document_buffers(&document, &base_path, &mut blob)?;
        let _images = extract_document_images(&document, &base_path, &buffers)?;

        Ok(())
    }
}

enum LoadUriScheme {
    #[allow(dead_code)]
    Base64Data,
    Relative,
}

fn parse_uri(uri: &str) -> LoadUriScheme {
    if uri.contains(':') {
        unimplemented!()
    } else {
        LoadUriScheme::Relative
    }
}

fn extract_uri(base_path: &PathBuf, uri: &str) -> anyhow::Result<Vec<u8>> {
    match parse_uri(uri) {
        LoadUriScheme::Base64Data => unimplemented!(),
        LoadUriScheme::Relative => {
            let path = base_path.join(PathBuf::from(uri));
            read_file_all(&path)
        }
    }
}

fn read_file_all(path: &PathBuf) -> anyhow::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::with_capacity((file.metadata()?.len() + 1) as usize);
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn extract_document_buffers(doc: &Document, base_path: &PathBuf, inline_blob: &mut Option<Vec<u8>>) -> anyhow::Result<Vec<Bytes>> {
    let mut extracted_buffers = Vec::with_capacity(doc.buffers().count());

    for buffer in doc.buffers() {
        let bytes = match buffer.source() {
            BufferSource::Uri(uri) => {
                Ok(extract_uri(base_path, uri)?)
            },
            BufferSource::Bin => {
                inline_blob.take().ok_or(Error::MissingBlob)
            }
        }?;

        let bytes = Bytes::from(bytes);
        extracted_buffers.push(bytes);
    }

    Ok(extracted_buffers)
}

fn extract_document_images<'a>(doc: &Document, base_path: &PathBuf, byte_buffers: &'a [Bytes]) -> anyhow::Result<Vec<Bytes>> {
    let mut extracted_images = Vec::with_capacity(doc.buffers().count());
    
    for image in doc.images() {
        let bytes = match image.source() {
            ImageSource::Uri { uri, mime_type: _ } => {
                glog::debug!("Image uri: {:?}", uri);
                Bytes::from(extract_uri(base_path, uri)?)
            },
            ImageSource::View { view, mime_type: _ } => {
                let bytes = &byte_buffers[view.buffer().index()];
                let beg = view.offset();
                let end = beg + view.length();
                
                bytes.slice(beg..end)
            }
        };

        extracted_images.push(bytes);
    }

    Ok(extracted_images)
}