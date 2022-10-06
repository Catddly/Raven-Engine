mod pack_unpack;
pub mod loader;

use std::{marker::PhantomData, fmt::Debug, any::{Any, TypeId}};

use crate::container::{TreeByteBuffer, TreeByteBufferNode};
use pack_unpack::*;

pub enum AssetType {
    Mesh,
}

impl Debug for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Mesh => write!(f, "Mesh Asset")
        }
    }
}

pub trait RawAsset {
    fn asset_type(&self) -> AssetType;

    fn as_any(&self) -> &dyn Any;
}

/// Downcast have 1us or 2us overhead
pub fn try_downcast_asset<T: 'static>(raw_asset: &Box<dyn RawAsset>) -> anyhow::Result<&T> {
    // do a double check here
    // we can use downcast_ref_unchecked() instead, but this is a unstable features, so we use downcast_ref()
    match raw_asset.asset_type() {
        AssetType::Mesh => {
            if TypeId::of::<T>() != TypeId::of::<Mesh::Raw>() {
                anyhow::bail!("Try to downcast mesh raw asset to invalid raw asset type!");
            }

            Ok(raw_asset.as_any().downcast_ref::<T>().unwrap())
        },
    }
}

macro_rules! define_asset {
    // pack Vec (compound type)
    (@packed_func $out:expr; $field:expr; Vec($($type:tt)+)) => {
        let mut new_node = TreeByteBufferNode::new();
        new_node.patch_addr = packed_vec_header(&mut $out.bytes, $field.len() as u64);

        for elem in $field.iter() {
            define_asset!(@packed_func new_node.buffer; elem; $($type)+);
        }

        $out.childs.push(new_node);
    };
    // specified Vec type (compound type)
    (@asset_ty Vec($($type:tt)+)) => {
        Vec<define_asset!(@asset_ty $($type)+)>
    };
    // specified packed Vec type (compound type)
    (@packed_ty Vec($($type:tt)+)) => {
        FlatVec<define_asset!(@packed_ty $($type)+)>
    };
    // user read back type used in FieldReader
    (@read_ty Vec($($type:tt)+)) => {
        &'a [$($type)+]
    };
    // user read back pub field function
    (@read_func $field_name:ident; Vec($($type:tt)+)) => {
        pub fn $field_name<'a>(&self) -> define_asset!(@read_ty Vec($($type)+)) {
            unsafe {
                let field_addr = std::ptr::addr_of!((*self.base_addr).$field_name);
                read_flat_vec(field_addr)
            }
        }
    };

    // pack plain type
    (@packed_func $out:expr; $field:expr; $($type:tt)+) => {
        packed_plain_field(&mut $out.bytes, $field)
    };
    // expand asset origin plain field types
    (@asset_ty $($type:tt)+) => {
        $($type)+
    };
    // expand asset packed plain field types
    (@packed_ty $($type:tt)+) => {
        $($type)+
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
            #[derive($($derive:tt)+)]
        )?
        $struct_name:ident {
            $(
                $field_name:ident { $($type:tt)+ }
            )+
        }
    ) => {
        #[allow(non_snake_case)]
        pub mod $struct_name {
            use super::*;

            $(#[derive($($derive)+)])?
            pub struct Raw {
                $(
                    pub $field_name: define_asset!(@asset_ty $($type)+),
                )+
            }

            impl Raw {
                pub fn write_packed(&self, writer: &mut impl std::io::Write) {
                    let mut byte_buffer = TreeByteBuffer::new();

                    // expand fields to pack functions
                    $(
                        define_asset!(@packed_func byte_buffer; &self.$field_name; $($type)+);
                    )+

                    byte_buffer.write_packed(writer);
                }
            }

            impl RawAsset for Raw {
                fn asset_type(&self) -> AssetType {
                    AssetType::$struct_name
                }

                fn as_any(&self) -> &dyn Any {
                    self
                }
            }

            #[repr(packed)]
            pub struct Packed {
                $(
                    $field_name: define_asset!(@packed_ty $($type)+),
                )+
            }

            // TODO: hide the raw ptr
            pub fn get_field_reader(base_addr: *const $struct_name::Packed) -> $struct_name::FieldReader {
                FieldReader::from_raw_ptr(base_addr)
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
                    define_asset!(@read_func $field_name; $($type)+);
                )+
            }
        }
    };
}

define_asset!{
    #[derive(Default, Debug)]
    Mesh {
        positions { Vec([f32; 3]) }
        normals { Vec([f32; 3]) }
        colors { Vec([f32; 4]) }
        uvs { Vec([f32; 2]) }
        tangents { Vec([f32; 4]) }
        material_ids { Vec(u32) }
        indices { Vec(u32) }
    }
}