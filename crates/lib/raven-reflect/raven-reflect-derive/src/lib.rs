extern crate proc_macro;

mod field_attributes;
mod trait_attributes;

mod reflect_meta;
mod reflect_gen;

mod crate_manifest;
mod quoted;
mod primitive_parser;

use proc_macro::TokenStream;
use reflect_gen::{gen_struct, gen_primitives};
use reflect_meta::{ReflectTypeMetaInfo, ReflectMeta};
use syn::{parse_macro_input, DeriveInput};

pub(crate) static REFLECT_ATTR: &str = "reflect";
pub(crate) static REFLECT_VALUE_ATTR: &str = "reflect_value";

#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    let reflect_meta = match ReflectTypeMetaInfo::from_derive_input(&derive_input) {
        Ok(meta) => meta,
        Err(err) => return err.into_compile_error().into(),
    };

    match reflect_meta {
        ReflectTypeMetaInfo::Struct(meta) => gen_struct(&meta),
        ReflectTypeMetaInfo::Primitive(meta) => gen_primitives(&meta),
    }
}

/// Macro to generate Reflect implementations for primitive types.
/// 
/// Since we cannot alter the foreign types (e.g. primitive types (u32, u16, etc.) and std types (String, Vec, etc.)),
/// we cannot use #[derive(Reflect)] to implement Reflect for them.
/// But we can use function-like procedural macro to generate implementations for us. 
#[proc_macro]
pub fn impl_reflect_primitive(input: TokenStream) -> TokenStream {
    let parser = parse_macro_input!(input as primitive_parser::PrimitiveParser);

    let meta = ReflectMeta::new(
        &parser.type_name,
        &parser.generics,
        parser.traits.unwrap_or_default(),
    );

    gen_primitives(&meta)
}