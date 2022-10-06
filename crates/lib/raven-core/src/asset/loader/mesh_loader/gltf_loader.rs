use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use gltf::{Gltf, Document, buffer::Source as BufferSource, Error, image::Source as ImageSource, mesh::Mode};
use bytes::Bytes;
use glam::{Mat4, Vec3, Vec4};

use crate::{filesystem::{self, ProjectFolder}, asset::{loader::loader::LoadAssetMeshType, Mesh, RawAsset}};
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
    fn load(&self) -> anyhow::Result<Box<dyn RawAsset>> {
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
        let images = extract_document_images(&document, &base_path, &buffers)?;

        let raw_asset = load_gltf_default_scene(&document, &buffers, &images)?;

        Ok(Box::new(raw_asset))
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

fn load_gltf_node<F>(node: &gltf::Node, universal_xform: Mat4, func: &mut F) -> anyhow::Result<()>
where
    F: FnMut(&gltf::Node, Mat4) -> anyhow::Result<()>,
{
    let node_local_xform = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global_xform = universal_xform * node_local_xform;

    func(&node, global_xform)?;

    for child in node.children() {
        load_gltf_node(&child, global_xform, func)?;
    }

    Ok(())
}

fn load_gltf_default_scene(doc: &Document, buffers: &[Bytes], _images: &[Bytes]) -> anyhow::Result<Mesh::Raw> {
    let scene = doc.default_scene().ok_or(anyhow::anyhow!("Failed to load default scene from gltf!"))?;

    glog::debug!("Loading gltf scene: {}", scene.name().unwrap());

    let universal_trans = Mat4::IDENTITY;
    let mut raw_mesh = Mesh::Raw::default();

    let mut read_node_func = |node: &gltf::Node, transform: Mat4| -> anyhow::Result<()>  {
        // only load mesh data right now
        if let Some(mesh) = node.mesh() {
            glog::debug!("Loading gltf mesh: {}", scene.name().unwrap());
            
            for prim in mesh.primitives() {
                // TODO: load material

                // only support triangles for now
                assert!(matches!(prim.mode(), Mode::Triangles));

                let attrib_reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));
            
                // must have position
                let positions = if let Some(iter) = attrib_reader.read_positions() {
                    iter.collect::<Vec<_>>()
                } else {
                    anyhow::bail!("Gltf mesh must have position attribute!");
                };

                // must have normal
                let normals = if let Some(iter) = attrib_reader.read_normals() {
                    iter.collect::<Vec<_>>()
                } else {
                    anyhow::bail!("Gltf mesh must have position attribute!");
                };

                // optional color
                let mut colors= if let Some(iter) = attrib_reader.read_colors(0) {
                    iter.into_rgba_f32().collect::<Vec<_>>()
                } else {
                    vec![[1.0, 1.0, 1.0, 1.0]; positions.len()]
                };

                // optional uv
                let (mut uvs, found_uv) = if let Some(iter) = attrib_reader.read_tex_coords(0) {
                    (iter.into_f32().collect::<Vec<_>>(), true) 
                } else {
                    (vec![[0.0, 0.0]; positions.len()], false)
                };

                // optional tangent
                let (mut tangents, found_tangent) = if let Some(iter) = attrib_reader.read_tangents() {
                    (iter.collect::<Vec<_>>(), true) 
                } else {
                    (vec![[0.0, 0.0, 0.0, 0.0]; positions.len()], false)
                };

                // material id for this every vertex
                // TODO: temporary, assume only one material is used for this whole mesh
                let mut material_ids = vec![0; positions.len()];

                let mut indices = if let Some(indices) = attrib_reader.read_indices() {
                    indices.into_u32().collect::<Vec<_>>()
                } else {
                    // get indices from positions
                    // assume that each three vertices construct into a triangle
                    (0..positions.len() as u32).collect::<Vec<_>>()
                };

                // if transform matrix's determinant is negative, we need to flip the triangle winding order
                // see https://math.stackexchange.com/questions/3832660/what-does-it-mean-by-negative-determinant for explanation
                let flip_winding_order = transform.determinant() < 0.0;

                if flip_winding_order {
                    for face_prim in indices.chunks_exact_mut(3) {
                        // flip winding order clockwise back to counter-clockwise
                        face_prim.swap(0, 2);
                    }
                }

                // if this mesh contain uv, we assume it have textures on it.
                // so we need tangents to build TBN matrix for normal mapping etc.
                if found_uv && !found_tangent {
                    glog::info!("Generating tangents for texture mesh!");
                    mikktspace::generate_tangents(&mut GenTangentContext {
                        positions: positions.as_slice(),
                        normals: normals.as_slice(),
                        uvs: uvs.as_slice(),
                        indices: indices.as_slice(),
                        tangents: tangents.as_mut_slice(),
                    });
                }

                // combine submesh
                {
                    // offset by base index
                    let index_offset = raw_mesh.positions.len() as u32;
                    let mut indices = indices.into_iter()
                        .map(|idx| idx + index_offset)
                        .collect::<Vec<_>>();
    
                    raw_mesh.indices.append(&mut indices);
                    raw_mesh.colors.append(&mut colors);
                    raw_mesh.uvs.append(&mut uvs);
                    raw_mesh.material_ids.append(&mut material_ids);

                    // pre-transform vertex into transformed position
                    let mut positions = positions.into_iter()
                        // position vector
                        .map(|pos| (transform * Vec3::from(pos).extend(1.0)).truncate().to_array())
                        .collect::<Vec<_>>();
                    raw_mesh.positions.append(&mut positions);

                    // TODO: inverse transpose of matrix?
                    let mut normals = normals.into_iter()
                        // direction vector
                        .map(|n| (transform * Vec3::from(n).extend(0.0)).truncate().normalize().to_array())
                        .collect::<Vec<_>>();
                    raw_mesh.normals.append(&mut normals);

                    let mut tangents = tangents.into_iter()
                        // direction vector
                        .map(|n| {
                            let origin = Vec4::from(n);
                            let n = (transform * origin.truncate().extend(0.0)).truncate().normalize();
                            // flip tangent to opposite direction by flip w component (homogeneous coordinate)
                            n.extend(origin.w * if flip_winding_order { -1.0 } else { 1.0 }).into()
                        })
                        .collect::<Vec<_>>();
                    raw_mesh.tangents.append(&mut tangents);
                }
            }
        }
        Ok(())
    };

    for node in scene.nodes() {
        load_gltf_node(&node, universal_trans, &mut read_node_func)?;
    }

    Ok(raw_mesh)
}

struct GenTangentContext<'a> {
    positions: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    uvs: &'a [[f32; 2]],
    indices: &'a [u32],
    tangents: &'a mut [[f32; 4]],
}

impl<'a> GenTangentContext<'a> {
    #[inline(always)]
    fn base_index(&self, face: usize, vert: usize) -> usize {
        self.indices[face * 3 + vert] as usize
    }
}

impl<'a> mikktspace::Geometry for GenTangentContext<'a> {
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.base_index(face, vert)]
    }

    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.base_index(face, vert)]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.base_index(face, vert)]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        // stick tangent back
        self.tangents[self.base_index(face, vert)] = tangent;
    }
}