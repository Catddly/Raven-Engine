pub mod loader;
mod asset_process;
pub mod asset_manager;
mod pack_unpack;
mod util;
mod error;

use std::{marker::PhantomData, fmt::Debug, path::PathBuf};

use bytes::Bytes;
use unsafe_any::UnsafeAny;
use turbosloth::Lazy;

use crate::container::{TreeByteBuffer, TreeByteBufferNode};
use crate::asset::asset_manager::AssetHandle;
use pack_unpack::*;

pub enum AssetType {
    Vacant,
    Mesh,
    Material,
    Texture,
}

impl Debug for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Vacant => write!(f, "Vacant Asset"),
            AssetType::Mesh => write!(f, "Mesh Asset"),
            AssetType::Material => write!(f, "Material Asset"),
            AssetType::Texture => write!(f, "Image Asset"),
        }
    }
}

pub trait RawAsset {
    fn asset_type(&self) -> AssetType;

    fn as_any(&self) -> &dyn UnsafeAny;

    // All the downcast functions.
    // Downcast have 1~2 us overhead.
    // 
    // Try to downcast the asset without the overhead, because we ensure the type in compile-time.
    // We do not want to pay for the things we don't use.
    // We can use downcast_ref_unchecked() instead, but this is a unstable features, so we use UnsafeAny to bypass the runtime check.
    fn as_mesh(&self) -> Option<&Mesh::Raw> {
        match self.asset_type() {
            AssetType::Mesh => Some(unsafe { self.as_any().downcast_ref_unchecked::<Mesh::Raw>() }),
            _ => None,
        }
    }

    fn as_material(&self) -> Option<&Material::Raw> {
        match self.asset_type() {
            AssetType::Material => Some(unsafe { self.as_any().downcast_ref_unchecked::<Material::Raw>() }),
            _ => None,
        }
    }

    fn as_texture(&self) -> Option<&Texture::Raw> {
        match self.asset_type() {
            AssetType::Texture => Some(unsafe { self.as_any().downcast_ref_unchecked::<Texture::Raw>() }),
            _ => None,
        }
    }
}

pub trait Asset {
    fn asset_type(&self) -> AssetType;

    fn as_any(&self) -> &dyn UnsafeAny;

    // All the downcast functions.
    // Downcast have 1~2 us overhead.
    // 
    // Try to downcast the asset without the overhead, because we ensure the type in compile-time.
    // We do not want to pay for the things we don't use.
    // We can use downcast_ref_unchecked() instead, but this is a unstable features, so we use UnsafeAny to bypass the runtime check.
    fn as_mesh(&self) -> Option<&Mesh::Raw> {
        match self.asset_type() {
            AssetType::Mesh => Some(unsafe { self.as_any().downcast_ref_unchecked::<Mesh::Raw>() }),
            _ => None,
        }
    }

    fn as_material(&self) -> Option<&Material::Raw> {
        match self.asset_type() {
            AssetType::Material => Some(unsafe { self.as_any().downcast_ref_unchecked::<Material::Raw>() }),
            _ => None,
        }
    }

    fn as_texture(&self) -> Option<&Texture::Raw> {
        match self.asset_type() {
            AssetType::Texture => Some(unsafe { self.as_any().downcast_ref_unchecked::<Texture::Raw>() }),
            _ => None,
        }
    }
}

/// Vacant asset.
/// To be used when update resources, or as a placeholder for asset registry.
pub struct VacantAsset {}

impl Asset for VacantAsset {
    fn asset_type(&self) -> AssetType {
        AssetType::Vacant
    }

    fn as_any(&self) -> &dyn UnsafeAny {
        self
    }
}

/// We can't make a length query function in the macro.
/// I know this design is weird, but it is the most efficient and easy way to get the length of a Vector Array's information.
pub enum VecArrayQueryParam {
    Index(usize),
    Length,
}

/// We can't make a length query function in the macro.
/// I know this design is weird, but it is the most efficient and easy way to get the length of a Vector Array's information.
pub enum VecArrayQueryResult<'a, T: Sized + Copy> {
    Length(usize),
    Value(T),
    Array(&'a [T])
}

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
        pub fn $field_name<'a>(&self, param: VecArrayQueryParam) -> VecArrayQueryResult<'a, AssetRef<$($type)+ ::Packed>> {
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
    // (@raw_ty Asset($($type:tt)+)) => {
    //     Lazy<define_asset!(@raw_ty $($type)+ ::Storage)>
    // };
    (@raw_ty Asset($($type:tt)+)) => {
        Lazy<AssetHandle>
    };
    // specified packed Asset type
    (@packed_ty Asset($($type:tt)+)) => {
        AssetRef<define_asset!(@packed_ty $($type)+ ::Packed)>
    };
    // pack Asset
    (@packed_func $out:expr; $field:expr; Asset($($type:tt)+)) => {
        // get identity value from Lazy
        // Lazy will hash the values of the type to get a UUID like hash.
        let asset_ref : AssetRef<$($type)+ ::Packed> = AssetRef {
            // TODO: notice that AssetHandle is order-dependent id allocation system,
            // so then identity of the Lazy handle will differ when we register the asset in different order.
            // this will cause issues when we try to load the packed assets back from disk, we may refer to different resources.
            identity: $field.identity(),
            _marker: PhantomData, 
        };
        pack_plain_field(&mut $out.bytes, &asset_ref)
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
                $field_name_asset:ident { $($type_asset:tt)+ }
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

                fn as_any(&self) -> &dyn UnsafeAny {
                    self
                }
            }

            $(#[derive($($derive_asset)+)])?
            pub struct Storage {
                $(
                    pub $field_name_asset: define_asset!(@raw_ty $($type_asset)+),
                )+
            }

            impl Storage {
                pub fn write_packed(&self, writer: &mut impl std::io::Write) {
                    let mut byte_buffer = TreeByteBuffer::new();

                    // expand fields to pack functions
                    $(
                        define_asset!(@packed_func byte_buffer; &self.$field_name_asset; $($type_asset)+);
                    )+

                    byte_buffer.write_packed(writer);
                }
            }

            impl Asset for Storage {
                fn asset_type(&self) -> AssetType {
                    AssetType::$asset_type
                }

                fn as_any(&self) -> &dyn UnsafeAny {
                    self
                }
            }

            #[repr(packed)]
            pub struct Packed {
                $(
                    $field_name_asset: define_asset!(@packed_ty $($type_asset)+),
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
                    define_asset!(@read_func $field_name_asset; $($type_asset)+);
                )+
            }
        }
    };
}

pub struct AssetRef<T> {
    // hash value on the asset
    identity: u64,
    // mark as invariant
    _marker: PhantomData<fn(&T)>,
}

impl<T> Clone for AssetRef<T> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            _marker: PhantomData,
        }
    }
}
impl<T> Copy for AssetRef<T> {}

impl<T> PartialEq for AssetRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl<T> Eq for AssetRef<T> {}

impl<T> std::hash::Hash for AssetRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.identity)
    }
}

impl<T> PartialOrd for AssetRef<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.identity.partial_cmp(&other.identity)
    }
}

impl<T> Ord for AssetRef<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.identity.cmp(&other.identity)
    }
}

impl<T> AssetRef<T> {
    pub fn identity(&self) -> u64 {
        self.identity
    }
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
    Source(PathBuf),
}

impl Default for TextureSource {
    fn default() -> Self {
        TextureSource::Empty
    }
}


#[derive(Clone, Hash, Debug)]
pub enum TextureGammaSpace {
    Srgb,
    Linear,
}

#[derive(Clone, Hash, Debug)]
pub struct TextureDesc {
    gamma_space: TextureGammaSpace,
    use_mipmap: bool,
}

impl Default for TextureDesc {
    fn default() -> Self {
        Self {
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