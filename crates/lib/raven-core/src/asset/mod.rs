mod asset_manager;

pub mod loader;
mod asset_process;
pub mod asset_registry;
mod asset_baker;
mod pack_unpack;
mod util;
mod error;

pub use asset_manager::{AssetManager, AssetLoadDesc};

use std::path::PathBuf;
use std::sync::Arc;
use std::{marker::PhantomData, fmt::Debug};

use downcast_rs::{impl_downcast, DowncastSync};
use bytes::Bytes;

use crate::container::{TreeByteBuffer, TreeByteBufferNode};
use crate::asset::asset_registry::{DiskAssetRef, AssetRef};
use crate::math::AABB;
use pack_unpack::*;

use self::asset_registry::AssetHandle;
use self::loader::{LoadAssetType};

pub trait TaggedAssetType {}

#[repr(u32)]
pub enum AssetType {
    Vacant,
    Baked,
    Mesh,
    Material,
    Texture,
}

impl Debug for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Vacant => write!(f, "Vacant Asset"),
            AssetType::Baked => write!(f, "Baked Asset"),
            AssetType::Mesh => write!(f, "Mesh Asset"),
            AssetType::Material => write!(f, "Material Asset"),
            AssetType::Texture => write!(f, "Image Asset"),
        }
    }
}

// pub fn as_raw_mesh_ref(raw_mesh: &Arc<dyn RawAsset>) -> Option<&Mesh::Raw> {
//     raw_mesh.downcast_ref()
// }

// TODO:
// All the downcast functions.
// Downcast have 1~2 us overhead.
// 
// Try to downcast the asset without the overhead, because we ensure the type in compile-time.
// We do not want to pay for the things we don't use.
// We can use downcast_ref_unchecked() instead, but this is a unstable features, so we use UnsafeAny to bypass the runtime check.

// DowncastSync already implemented Send + Sync for Asset
pub trait RawAsset: DowncastSync {
    fn asset_type(&self) -> AssetType;
}
impl_downcast!(sync RawAsset);

pub trait AsConcreteRawAsset {
    fn as_mesh(&self) -> Option<&Mesh::Raw>;
    fn as_texture(&self) -> Option<&Texture::Raw>;
    fn as_material(&self) -> Option<&Material::Raw>;
    fn as_baked(&self) -> Option<&BakedRawAsset>;
}

impl AsConcreteRawAsset for Arc<dyn RawAsset> {
    fn as_mesh(&self) -> Option<&Mesh::Raw> {
        self.downcast_ref::<Mesh::Raw>()
    }

    fn as_texture(&self) -> Option<&Texture::Raw> {
        self.downcast_ref::<Texture::Raw>()
    }

    fn as_material(&self) -> Option<&Material::Raw> {
        self.downcast_ref::<Material::Raw>()
    }

    fn as_baked(&self) -> Option<&BakedRawAsset> {
        self.downcast_ref::<BakedRawAsset>()
    }
}

impl AsConcreteRawAsset for Box<dyn RawAsset> {
    fn as_mesh(&self) -> Option<&Mesh::Raw> {
        self.downcast_ref::<Mesh::Raw>()
    }

    fn as_texture(&self) -> Option<&Texture::Raw> {
        self.downcast_ref::<Texture::Raw>()
    }

    fn as_material(&self) -> Option<&Material::Raw> {
        self.downcast_ref::<Material::Raw>()
    }

    fn as_baked(&self) -> Option<&BakedRawAsset> {
        self.downcast_ref::<BakedRawAsset>()
    }
}

// TODO:
// All the downcast functions.
// Downcast have 1~2 us overhead.
// 
// Try to downcast the asset without the overhead, because we ensure the type in compile-time.
// We do not want to pay for the things we don't use.
// We can use downcast_ref_unchecked() instead, but this is a unstable features, so we use UnsafeAny to bypass the runtime check.

// DowncastSync already implemented Send + Sync for Asset
pub trait Asset: DowncastSync {
    fn asset_type(&self) -> AssetType;
}
impl_downcast!(sync Asset);

pub trait AsConcreteAsset {
    fn as_mesh(&self) -> Option<&Mesh::Storage>;
    fn as_texture(&self) -> Option<&Texture::Storage>;
    fn as_material(&self) -> Option<&Material::Storage>;
    fn as_baked(&self) -> Option<&BakedAsset>;
}

impl AsConcreteAsset for Arc<dyn Asset> {
    fn as_mesh(&self) -> Option<&Mesh::Storage> {
        self.downcast_ref::<Mesh::Storage>()
    }

    fn as_texture(&self) -> Option<&Texture::Storage> {
        self.downcast_ref::<Texture::Storage>()
    }

    fn as_material(&self) -> Option<&Material::Storage> {
        self.downcast_ref::<Material::Storage>()
    }

    fn as_baked(&self) -> Option<&BakedAsset> {
        self.downcast_ref::<BakedAsset>()
    }
}

impl AsConcreteAsset for Box<dyn Asset> {
    fn as_mesh(&self) -> Option<&Mesh::Storage> {
        self.downcast_ref::<Mesh::Storage>()
    }

    fn as_texture(&self) -> Option<&Texture::Storage> {
        self.downcast_ref::<Texture::Storage>()
    }

    fn as_material(&self) -> Option<&Material::Storage> {
        self.downcast_ref::<Material::Storage>()
    }

    fn as_baked(&self) -> Option<&BakedAsset> {
        self.downcast_ref::<BakedAsset>()
    }
}

/// Vacant asset.
/// To be used when update resources, or as a placeholder for asset registry.
pub struct VacantAsset {}

impl Asset for VacantAsset {
    fn asset_type(&self) -> AssetType {
        AssetType::Vacant
    }
}

impl TaggedAssetType for VacantAsset {}

/// Baked asset.
/// To be used to index data from mmap.
pub struct BakedAsset {
    uri: PathBuf,
}

impl BakedAsset {
    pub fn origin_asset_type(&self) -> AssetType {
        let ty = loader::extract_asset_type(&self.uri);
        match ty {
            loader::LoadAssetType::Mesh(_) => AssetType::Mesh,
            loader::LoadAssetType::Texture(_) => AssetType::Texture,
            loader::LoadAssetType::Material(_) => AssetType::Material,
            _ => unimplemented!()
        }
    }
}

impl Asset for BakedAsset {
    fn asset_type(&self) -> AssetType {
        AssetType::Baked
    }
}

impl TaggedAssetType for BakedAsset {}

#[derive(Clone)]
pub struct BakedRawAsset {
    handle: AssetHandle,
}

impl BakedRawAsset {
    pub fn get_asset_handle(&self) -> AssetHandle {
        self.handle.clone()
    }
}

impl RawAsset for BakedRawAsset {
    fn asset_type(&self) -> AssetType {
        AssetType::Baked
    }
}

/// We can't make a length query function in the macro.
/// I know this design is weird, but it is the most efficient and easy way to get the length of a Vector Array's information.
pub enum VecArrayQueryParam {
    Index(usize),
    Length,
}

// Just some helper functions
impl VecArrayQueryParam {
    #[inline(always)]
    pub fn index(idx: usize) -> VecArrayQueryParam {
        VecArrayQueryParam::Index(idx)
    }

    #[inline(always)]
    pub fn length() -> VecArrayQueryParam {
        VecArrayQueryParam::Length
    }
}

/// We can't make a length query function in the macro.
/// I know this design is weird, but it is the most efficient and easy way to get the length of a Vector Array's information.
pub enum VecArrayQueryResult<'a, T: Sized + Copy> {
    Length(usize),
    Value(T),
    Array(&'a [T])
}

// Just some helper functions
impl<'a, T: Sized + Copy> VecArrayQueryResult<'a, T> {
    #[inline(always)]
    pub fn length(&self) -> usize {
        if let VecArrayQueryResult::Length(len) = self {
            *len
        } else {
            panic!("Try to get different query result!");
        }
    }

    #[inline(always)]
    pub fn value(&self) -> T {
        if let VecArrayQueryResult::Value(v) = self {
            *v
        } else {
            panic!("Try to get different query result!");
        }
    }

    #[inline(always)]
    pub fn array(&self) -> &'a [T] {
        if let VecArrayQueryResult::Array(arr) = self {
            *arr
        } else {
            panic!("Try to get different query result!");
        }
    }
}

// TODO: this is very hard to read and debug, turn this into procedural macros 
macro_rules! define_asset {
    // specified Vec type (compound type)
    (@raw_ty Vec($($type:tt)+)) => {
        Vec<define_asset!(@raw_ty $($type)+)>
    };
    // specified packed Vec type (compound type)
    (@packed_ty Vec($($type:tt)+)) => {
        FlatVec<define_asset!(@packed_ty $($type)+)>
    };
    // pack Vec (compound type)
    (@packed_func $out:expr; $field:expr; Vec($($type:tt)+)) => {
        let mut new_node = TreeByteBufferNode::new();
        new_node.patch_addr = pack_vec_header(&mut $out.bytes, $field.len() as u64);

        for elem in $field.iter() {
            define_asset!(@packed_func new_node.buffer; elem; $($type)+);
        }

        $out.childs.push(new_node);
    };
    // user read back pub field function
    (@read_func $field_name:ident; Vec(Vec($($type:tt)+))) => {
        pub fn $field_name<'a>(&self, param: VecArrayQueryParam) -> VecArrayQueryResult<'a, $($type)+> {
            unsafe {
                let field_addr = std::ptr::addr_of!((*self.base_addr).$field_name);
                let flat_vec = read_flat_vec(field_addr);

                match param {
                    VecArrayQueryParam::Length => {
                        VecArrayQueryResult::Length(flat_vec.len())
                    },
                    VecArrayQueryParam::Index(idx) => {             
                        assert!(idx < flat_vec.len());

                        let target_vec = &flat_vec[idx];
                        VecArrayQueryResult::Array(read_flat_vec(target_vec as *const FlatVec<$($type)+>))
                    },
                }
            }
        }
    };
    (@read_func $field_name:ident; Vec(Asset($($type:tt)+))) => {
        pub fn $field_name<'a>(&self, param: VecArrayQueryParam) -> VecArrayQueryResult<'a, DiskAssetRef<$($type)+ ::Packed>> {
            unsafe {
                let field_addr = std::ptr::addr_of!((*self.base_addr).$field_name);
                let flat_vec = read_flat_vec(field_addr);

                match param {
                    VecArrayQueryParam::Length => {
                        VecArrayQueryResult::Length(flat_vec.len())
                    },
                    VecArrayQueryParam::Index(idx) => {             
                        assert!(idx < flat_vec.len());

                        let target_vec = &flat_vec[idx];
                        VecArrayQueryResult::Value(target_vec.clone())
                    },
                }
            }
        }
    };
    (@read_func $field_name:ident; Vec($($type:tt)+)) => {
        pub fn $field_name<'a>(&self) -> &'a [$($type)+] {
            unsafe {
                let field_addr = std::ptr::addr_of!((*self.base_addr).$field_name);
                read_flat_vec(field_addr)
            }
        }
    };

    // specified Asset type
    (@raw_ty Asset($($type:tt)+)) => {
        AssetRef<$($type)+ ::Storage>
    };
    // specified packed Asset type
    (@packed_ty Asset($($type:tt)+)) => {
        DiskAssetRef<$($type)+  ::Packed>
    };
    // pack Asset
    (@packed_func $out:expr; $field:expr; Asset($($type:tt)+)) => {
        let disk_ref = $field.disk_ref::<$($type ::Packed)+>();
        pack_plain_field(&mut $out.bytes, &disk_ref)
    };

    // expand asset origin plain field types
    (@raw_ty $($type:tt)+) => {
        $($type)+
    };
    // expand asset packed plain field types
    (@packed_ty $($type:tt)+) => {
        $($type)+
    };
    // pack plain type
    (@packed_func $out:expr; $field:expr; $($type:tt)+) => {
        pack_plain_field(&mut $out.bytes, $field)
    };
    // user read back type used in FieldReader
    (@read_ty $($type:tt)+) => {
        $($type)+
    };
    // user read back pub field function
    (@read_func $field_name:ident; $($type:tt)+) => {
        pub fn $field_name(&self) -> define_asset!(@read_ty $($type)+) {
            unsafe {
                let field_addr = std::ptr::addr_of!((*self.base_addr).$field_name);
                field_addr.read_unaligned()
            }
        }
    };

    (
        $(
            #[derive($($derive_raw:tt)+)]
        )?
        $struct_name:ident {
            $(
                $field_name_raw:ident { $($type_raw:tt)+ }
            )+
        }
        $(
            #[derive($($derive_asset:tt)+)]
        )?
        {
            $(
                $field_name_storage:ident { $($type_storage:tt)+ }
            )+
        }
        $asset_type:ident
    ) => {
        #[allow(non_snake_case)]
        pub mod $struct_name {
            use super::*;

            $(#[derive($($derive_raw)+)])?
            pub struct Raw {
                $(
                    pub $field_name_raw: define_asset!(@raw_ty $($type_raw)+),
                )+
            }

            impl RawAsset for Raw {
                fn asset_type(&self) -> AssetType {
                    AssetType::$asset_type
                }
            }

            $(#[derive($($derive_asset)+)])?
            pub struct Storage {
                $(
                    pub $field_name_storage: define_asset!(@raw_ty $($type_storage)+),
                )+
            }

            impl Storage {
                pub fn write_packed(&self, writer: &mut impl std::io::Write) {
                    let mut byte_buffer = TreeByteBuffer::new();

                    // expand fields to pack functions
                    $(
                        define_asset!(@packed_func byte_buffer; &self.$field_name_storage; $($type_storage)+);
                    )+

                    byte_buffer.write_packed(writer);
                }
            }

            impl Asset for Storage {
                fn asset_type(&self) -> AssetType {
                    AssetType::$asset_type
                }
            }

            impl TaggedAssetType for Storage {}

            #[repr(packed)]
            pub struct Packed {
                $(
                    $field_name_storage: define_asset!(@packed_ty $($type_storage)+),
                )+
            }

            pub fn get_field_reader(base_addr: &[u8]) -> $struct_name::FieldReader {
                FieldReader::from_raw_ptr(base_addr as *const [u8] as *const $struct_name::Packed)
            }

            #[derive(Clone)]
            pub struct FieldReader {
                base_addr: *const $struct_name::Packed,
                _marker: PhantomData<*const $struct_name::Packed>,
            }

            impl FieldReader {
                fn from_raw_ptr(base_addr: *const $struct_name::Packed) -> Self {
                    Self {
                        base_addr,
                        _marker: PhantomData,
                    }
                }

                $(
                    define_asset!(@read_func $field_name_storage; $($type_storage)+);
                )+
            }
        }
    };
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PackedVertex {
    position: [f32; 3],
    normal: u32,
}

define_asset!{
    #[derive(Default, Clone)]
    // raw
    Mesh {
        positions    { Vec([f32; 3]) }
        normals      { Vec([f32; 3]) }
        colors       { Vec([f32; 4]) }
        uvs          { Vec([f32; 2]) }
        tangents     { Vec([f32; 4]) }
        indices      { Vec(u32) }
        
        materials         { Vec(Material::Raw) }
        material_textures { Vec(Texture::Raw) }
        material_ids      { Vec(u32) }
    }
    // storage
    {
        packed       { Vec(PackedVertex) }
        colors       { Vec([f32; 4]) }
        tangents     { Vec([f32; 4]) }
        uvs          { Vec([f32; 2]) }
        indices      { Vec(u32) }
        aabb         { AABB }
        
        materials         { Vec(Asset(Material)) }
        material_textures { Vec(Asset(Texture)) }
        material_ids      { Vec(u32) }
    }
    Mesh
}

#[derive(Debug, Hash, Clone)]
pub enum TextureSource {
    Empty,
    Placeholder([u8; 4]),
    Bytes(Bytes),
    //Source(PathBuf),
}

impl Default for TextureSource {
    fn default() -> Self {
        TextureSource::Empty
    }
}


#[derive(Copy, Clone, Hash, Debug)]
pub enum TextureGammaSpace {
    Srgb,
    Linear,
}

#[derive(Copy, Clone, Hash, Debug)]
pub struct TextureDesc {
    pub gamma_space: TextureGammaSpace,
    pub use_mipmap: bool,
}

impl Default for TextureDesc {
    fn default() -> Self {
        Self {
            //extent: [1, 1, 1],
            //ty: LoadAssetTextureType::Unknown,
            gamma_space: TextureGammaSpace::Linear,
            use_mipmap: false,
        }
    }
}

define_asset!{
    // raw
    #[derive(Default, Debug, Clone, Hash)]
    Texture {
        source     { TextureSource }
        desc       { TextureDesc }
    }
    // storage
    #[derive(Default, Debug, Clone)]
    {
        extent     { [u32; 3] }
        lod_groups { Vec(Vec(u8)) }
        desc       { TextureDesc }
    }
    Texture
}

define_asset!{
    // raw
    #[derive(Default, Copy, Clone, Debug)]
    Material {
        metallic          { f32 }
        roughness         { f32 }
        base_color        { [f32; 4] }
        emissive          { [f32; 3] }
        texture_mapping   { [u32; 4] }      // textures to be used in this material [albedo, normal, specular, emissive]
        texture_transform { [[f32; 6]; 4] } // the corresponding 2D transform of the texture
    }
    // storage
    #[derive(Default, Copy, Clone, Debug)]
    {
        metallic          { f32 }
        roughness         { f32 }
        base_color        { [f32; 4] }
        emissive          { [f32; 3] }
        texture_mapping   { [u32; 4] }      // textures to be used in this material [albedo, normal, specular, emissive]
        texture_transform { [[f32; 6]; 4] } // the corresponding 2D transform of the texture
    }
    Material
}

fn get_uri_bake_stem(uri: &PathBuf) -> PathBuf {
    let asset_ty = loader::extract_asset_type(uri);

    let baked_uri = match asset_ty {
        LoadAssetType::Mesh(_) => {
            uri.strip_prefix("mesh/")
                .expect(format!("Incorrect mesh uri: {:?}", uri).as_str())
                .to_owned()
        }
        LoadAssetType::Texture(_) => {
            uri.strip_prefix("texture/")
                .expect(format!("Incorrect texture uri: {:?}", &uri).as_str())
                .to_owned()
        }
        _ => unimplemented!()
    }.to_string_lossy().to_string();

    PathBuf::from(baked_uri.replace("/", "=!"))
}

#[test]
fn test_vec_array_pack_unpack() {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use once_cell::sync::Lazy;
    use parking_lot::Mutex;

    let texture = Texture::Storage {
        extent: [1280, 1280, 1],
        lod_groups: vec![
            vec![8, 5, 4, 6, 1, 2, 4, 8],
            vec![3, 8, 7, 7],
            vec![3, 1],
            vec![5]
        ],
        desc: TextureDesc {
            gamma_space: TextureGammaSpace::Linear,
            use_mipmap: false,
        }
    };

    let mut file = std::fs::File::create("vec.bin").unwrap();
    texture.write_packed(&mut file);

    static ASSET_MMAPS: Lazy<Mutex<HashMap<PathBuf, memmap2::Mmap>>> = Lazy::new(|| {
        Mutex::new(HashMap::new())
    });

    // read back using memory mapped buffer
    let mut asset_map = ASSET_MMAPS.lock();
    let field_reader;
    {
        let data: &[u8] = {
            asset_map.entry(PathBuf::from("vec.bin")).or_insert_with(|| {
                let file = std::fs::File::open("vec.bin").unwrap();
    
                unsafe { memmap2::MmapOptions::new().map(&file).unwrap() }
            })
        };
    
        field_reader = Texture::get_field_reader(data);
    }

    assert_eq!(field_reader.extent(), [1280, 1280, 1]);
    let length = if let VecArrayQueryResult::Length(len) = field_reader.lod_groups(VecArrayQueryParam::Length) {
        len
    } else {
        0
    };
    println!("lod_groups length: {:?}", length);

    for i in 0..length {
        let lod = if let VecArrayQueryResult::Array(arr) = field_reader.lod_groups(VecArrayQueryParam::Index(i)) {
            arr.to_vec()
        } else {
            panic!("Wrong result type!")
        };

        println!("{:?}", lod);
        assert_eq!(texture.lod_groups[i], lod);
    }
}