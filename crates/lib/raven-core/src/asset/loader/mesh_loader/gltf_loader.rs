use std::{
    fs,
    io::{self, Read},
    path::PathBuf, sync::Arc,
};

use gltf::{Gltf, Document, buffer::Source as BufferSource, Error, image::Source as ImageSource, mesh::Mode, Material as GltfMaterial, texture::TextureTransform};
use bytes::Bytes;
use glam::{Mat4, Vec3, Vec4};

use crate::{filesystem::{self, ProjectFolder}, asset::{loader::loader::LoadAssetMeshType, Mesh, RawAsset, Material, TextureDesc, TextureGammaSpace, Texture, TextureSource}};
use super::super::loader::{self, AssetLoader};
use crate::asset::util::GenTangentContext;

pub struct GltfMeshLoader {
    path: PathBuf,
}

impl GltfMeshLoader {
    pub fn new(path: PathBuf) -> Self {
        let mesh_type = loader::extract_mesh_type(&path).unwrap();
        assert!(matches!(mesh_type, LoadAssetMeshType::Gltf), "Loading gltf resource but found other: {:?}", mesh_type);

        Self {
            path,
        }
    }
}

impl AssetLoader for GltfMeshLoader {
    fn load(&self) -> anyhow::Result<Arc<dyn RawAsset + Send + Sync>> {
        let dir = filesystem::get_project_folder_path_absolute(ProjectFolder::Assets)?;
        let path = dir.join(self.path.clone());
        assert!(path.is_file(), "Path may not exists or this path is not a file! {:?}", path);

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

        Ok(Arc::new(raw_asset))
    }

    fn get_load_uri(&self) -> PathBuf {
        self.path.clone()
    }
}

enum LoadUriScheme<'a> {
    Base64Data(Option<&'a str>, &'a str),
    Relative,
    Unsupported
}

fn parse_uri(uri: &str) -> LoadUriScheme {
    if uri.contains(':') {
        #[allow(clippy::manual_strip)]
        #[allow(clippy::iter_nth_zero)]
        if uri.starts_with("data:") {
            let first_match = &uri["data:".len()..].split(";base64,").nth(0);
            let second_match = &uri["data:".len()..].split(";base64,").nth(1);
            if second_match.is_some() {
                LoadUriScheme::Base64Data(Some(first_match.unwrap()), second_match.unwrap())
            } else if first_match.is_some() {
                LoadUriScheme::Base64Data(None, first_match.unwrap())
            } else {
                LoadUriScheme::Unsupported
            }
        } else {
            unimplemented!()
        }
    } else {
        LoadUriScheme::Relative
    }
}

fn extract_uri(base_path: &PathBuf, uri: &str) -> anyhow::Result<Vec<u8>> {
    match parse_uri(uri) {
        LoadUriScheme::Base64Data(_, base64_str) => {
            base64::decode(base64_str).map_err(|err| err.into())
        }
        LoadUriScheme::Relative => {
            let path = base_path.join(PathBuf::from(uri));
            read_file_all(&path)
        }
        LoadUriScheme::Unsupported => {
            panic!("Unsupported uri!")
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

fn load_gltf_material(mat: &GltfMaterial, images: &[Bytes]) -> anyhow::Result<(Vec<Texture::Raw>, Material::Raw)> {
    const DEFAULT_TEX_XFORM : [f32; 6] = [
        1.0, 0.0,
        0.0, 1.0,
        0.0, 0.0
    ];

    fn texture_transform_to_matrix(xform: Option<TextureTransform>) -> [f32; 6] {
        if let Some(xform) = xform {
            let r = xform.rotation();
            let s = xform.scale();
            let o = xform.offset();

            [
                 r.cos() * s[0], r.sin() * s[1],
                -r.sin() * s[0], r.cos() * s[1],
                 o[0],           o[1]
            ]
        } else {
            DEFAULT_TEX_XFORM
        }
    }

    let (albedo_tex, albedo_tex_xform) = mat.pbr_metallic_roughness()
        .base_color_texture()
        .or_else(|| mat.pbr_specular_glossiness()?.diffuse_texture())
        .map_or(
            (Texture::Raw {
                source: TextureSource::Placeholder([255, 255, 255, 255]),
                desc: Default::default(),
            }, DEFAULT_TEX_XFORM),
            |tex|{
                let xform = texture_transform_to_matrix(tex.texture_transform());
                let img_bytes = images[tex.texture().source().index()].clone();

                (Texture::Raw {
                    source: TextureSource::Bytes(img_bytes),
                    desc: TextureDesc {
                        gamma_space: TextureGammaSpace::Srgb,
                        use_mipmap: true,
                        ..Default::default()
                    },
                }, xform)
            }
        );

    let normal_tex = mat
        .normal_texture()
        .map_or(Texture::Raw { 
            source: TextureSource::Placeholder([255, 255, 255, 255]),
            desc: Default::default(),
        }, 
        |tex| {
            let img_bytes = images[tex.texture().source().index()].clone();

            Texture::Raw {
                source: TextureSource::Bytes(img_bytes),
                desc: TextureDesc {
                    gamma_space: TextureGammaSpace::Linear,
                    use_mipmap: true,
                    ..Default::default()
                },
            }
        });

    let (specular_tex, specular_tex_xform) = mat
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
        .map_or((Texture::Raw { 
                source: TextureSource::Placeholder([255, 0, 255, 255]),
                desc: Default::default(),
            }, DEFAULT_TEX_XFORM),
            |tex| {
                let xform = texture_transform_to_matrix(tex.texture_transform());
                let img_bytes = images[tex.texture().source().index()].clone();

                (Texture::Raw {
                    source: TextureSource::Bytes(img_bytes),
                    desc: TextureDesc {
                        gamma_space: TextureGammaSpace::Linear,
                        use_mipmap: true,
                        ..Default::default()
                    },
                }, xform)
            }
        );

    let (emissive_tex, emissive_tex_xform) = mat
        .emissive_texture()
        .map_or((Texture::Raw { 
                source: TextureSource::Placeholder([255, 255, 255, 255]),
                desc: Default::default(),
            }, DEFAULT_TEX_XFORM),
            |tex| {
                let xform = texture_transform_to_matrix(tex.texture_transform());
                let img_bytes = images[tex.texture().source().index()].clone();

                (Texture::Raw {
                    source: TextureSource::Bytes(img_bytes),
                    desc: TextureDesc {
                        gamma_space: TextureGammaSpace::Srgb,
                        use_mipmap: true,
                        ..Default::default()
                    },
                }, xform)
            }
        );

    let material = Material::Raw {
        metallic: mat.pbr_metallic_roughness().metallic_factor(),
        roughness: mat.pbr_metallic_roughness().roughness_factor(),
        base_color: mat.pbr_metallic_roughness().base_color_factor(),
        emissive: mat.emissive_factor(),
        texture_mapping: [0, 1, 2, 3],
        texture_transform: [albedo_tex_xform, DEFAULT_TEX_XFORM, specular_tex_xform, emissive_tex_xform],
    };

    Ok((vec![albedo_tex, normal_tex, specular_tex, emissive_tex], material))
}

fn load_gltf_default_scene(doc: &Document, buffers: &[Bytes], images: &[Bytes]) -> anyhow::Result<Mesh::Raw> {
    let scene = doc.default_scene().ok_or(anyhow::anyhow!("Failed to load default scene from gltf!"))?;

    let universal_trans = Mat4::IDENTITY;
    let mut raw_mesh = Mesh::Raw::default();

    let mut read_node_func = |node: &gltf::Node, transform: Mat4| -> anyhow::Result<()>  {
        // only load mesh data right now
        if let Some(mesh) = node.mesh() {
            for prim in mesh.primitives() {
                // load material
                let (mut textures, mut material) = load_gltf_material(&prim.material(), images)?;

                let current_material_id = raw_mesh.materials.len() as u32;
                // offset material index by textures
                let texture_base = raw_mesh.material_textures.len() as u32;
                for map in material.texture_mapping.iter_mut() {
                    *map += texture_base;
                }

                raw_mesh.materials.push(material);
                raw_mesh.material_textures.append(&mut textures);

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
                let mut material_ids = vec![current_material_id; positions.len()];

                let mut indices = if let Some(indices) = attrib_reader.read_indices() {
                    indices.into_u32().collect::<Vec<_>>()
                } else {
                    // only support triangles for now
                    assert!(matches!(prim.mode(), Mode::Triangles));

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
                    glog::trace!("Generating tangents for texture mesh!");
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
                            let n = (transform * origin.truncate().extend(0.0)).truncate().normalize_or_zero();
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

#[test]
fn test_asset_load_gltf() {
    use crate::filesystem;

    filesystem::set_custom_mount_point(filesystem::ProjectFolder::Assets, "../../../resource/assets/").unwrap();

    let loader: Box<dyn AssetLoader> = Box::new(GltfMeshLoader::new(std::path::PathBuf::from("mesh/cube.glb")));
    let asset = loader.load().unwrap();
    println!("Load in {:?}", asset.asset_type());
    
    let now = std::time::Instant::now();
    let mesh_asset = asset.as_mesh().unwrap();
    println!("Cast time {:?} ns", now.elapsed().as_nanos());

    println!("{:?}", mesh_asset.positions);
    println!("{:?}", mesh_asset.material_ids);
    println!("{:#?}", mesh_asset.materials);
    println!("{:#?}", mesh_asset.material_textures);
}