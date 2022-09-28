mod pack_unpack;

use std::marker::PhantomData;

use crate::container::{TreeByteBuffer, TreeByteBufferNode};
use pack_unpack::*;

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
        $struct_name:ident {
            $(
                $field_name:ident { $($type:tt)+ }
            )+
        }
    ) => {
        #[allow(non_snake_case)]
        pub mod $struct_name {
            use super::*;

            pub struct Asset {
                $(
                    pub $field_name: define_asset!(@asset_ty $($type)+),
                )+
            }

            impl Asset {
                pub fn write_packed(&self, writer: &mut impl std::io::Write) {
                    let mut byte_buffer = TreeByteBuffer::new();

                    // expand fields to pack functions
                    $(
                        define_asset!(@packed_func byte_buffer; &self.$field_name; $($type)+);
                    )+

                    byte_buffer.write_packed(writer);
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

// TODO: remove this
define_asset!{
    Test {
        field_0 { u16 }
        field_1 { u32 }
        field_2 { u64 }
        field_3 { Vec(u64) }
    }
}