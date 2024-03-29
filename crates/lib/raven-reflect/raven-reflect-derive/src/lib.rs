extern crate proc_macro;

mod field_attributes;
mod trait_attributes;

mod reflect_meta;
mod reflect_gen;
mod from_reflect_gen;

mod crate_manifest;
mod quoted;
mod primitive_parser;

use from_reflect_gen::{from_gen_tuple_struct, from_gen_struct, from_gen_enum, from_gen_primitive, gen_from_reflect_primitives};
use proc_macro::TokenStream;
use reflect_gen::{gen_struct, gen_tuple_struct, gen_primitive, gen_enum};
use reflect_meta::{ReflectTypeMetaInfo, ReflectMeta};
use syn::{parse_macro_input, DeriveInput};

pub(crate) static REFLECT_ATTR: &str = "reflect";
pub(crate) static REFLECT_PRIM_ATTR: &str = "reflect_prim";

#[proc_macro_derive(Reflect, attributes(reflect, reflect_prim))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let reflect_meta = match ReflectTypeMetaInfo::from_derive_input(&ast) {
        Ok(meta) => meta,
        Err(err) => return err.into_compile_error().into(),
    };

    match reflect_meta {
        ReflectTypeMetaInfo::Struct(meta) | ReflectTypeMetaInfo::UnitStruct(meta) => gen_struct(&meta),
        ReflectTypeMetaInfo::TupleStruct(meta) => gen_tuple_struct(&meta),
        ReflectTypeMetaInfo::Enum(meta) => gen_enum(&meta),
        ReflectTypeMetaInfo::Primitive(meta) => gen_primitive(&meta),
    }
}

/// Derives the `FromReflect` trait.
///
/// This macro supports the following field attributes:
/// * `#[reflect(ignore)]`: Ignores the field. This requires the field to implement [`Default`].
/// * `#[reflect(default)]`: If the field's value cannot be read, uses its [`Default`] implementation.
/// * `#[reflect(default = "some_func")]`: If the field's value cannot be read, uses the function with the given name.
///
#[proc_macro_derive(FromReflect, attributes(reflect))]
pub fn derive_from_reflect(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let reflect_meta = match ReflectTypeMetaInfo::from_derive_input(&ast) {
        Ok(meta) => meta,
        Err(err) => return err.into_compile_error().into(),
    };

    match reflect_meta {
        ReflectTypeMetaInfo::Struct(meta) | ReflectTypeMetaInfo::UnitStruct(meta) => from_gen_struct(&meta),
        ReflectTypeMetaInfo::TupleStruct(meta) => from_gen_tuple_struct(&meta),
        ReflectTypeMetaInfo::Enum(meta) => from_gen_enum(&meta),
        ReflectTypeMetaInfo::Primitive(meta) => from_gen_primitive(&meta),
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

    gen_primitive(&meta)
}

/// Macro to generate FromReflect implementations for primitive types.
#[proc_macro]
pub fn impl_from_reflect_primitive(input: TokenStream) -> TokenStream {
    let parser = parse_macro_input!(input as primitive_parser::PrimitiveParser);

    let meta = ReflectMeta::new(
        &parser.type_name,
        &parser.generics,
        parser.traits.unwrap_or_default(),
    );

    gen_from_reflect_primitives(&meta)
}