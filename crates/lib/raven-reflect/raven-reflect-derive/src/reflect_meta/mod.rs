mod array_meta;
mod struct_meta;
mod enum_meta;

use raven_core::result::{ResultFlattener, CombinableError};

pub(crate) use struct_meta::*;
pub(crate) use enum_meta::*;

use syn::{Ident, Generics, DeriveInput, Data, Fields, Meta, spanned::Spanned, Path, punctuated::Punctuated, Variant, token::Comma};

use crate::{
    field_attributes, 
    reflect_gen,
    trait_attributes::ReflectTraits,
    REFLECT_ATTR, REFLECT_PRIM_ATTR,
    crate_manifest::CrateManifest,
};

const RAVEN_REFLECT_CRATE_NAME: &str = "raven_reflect";

pub struct SynError {
    inner: syn::Error,
}

impl CombinableError for SynError {
    fn combine(&mut self, other: Self) {
        self.inner.combine(other.inner)
    }
}

impl From<syn::Error> for SynError {
    fn from(value: syn::Error) -> Self {
        Self {
            inner: value
        }
    }
}

pub(crate) enum ReflectTypeMetaInfo<'a> {
    Struct(StructMetaInfo<'a>),
    UnitStruct(StructMetaInfo<'a>),
    TupleStruct(StructMetaInfo<'a>),
    Enum(EnumMetaInfo<'a>),
    Primitive(ReflectMeta<'a>),
}

/// Metadata of all reflected types.
pub(crate) struct ReflectMeta<'a> {
    /// Reflected name of this type. (e.g. struct MyStruct {}, then ident = "MyStruct")
    type_name: &'a Ident,
    /// Generics of this type. (e.g. <T, U>)
    generics: &'a Generics,
    /// Derived traits of this type. (e.g. #[derive(Default, Debug)])
    traits: ReflectTraits,
    /// Cached crate path to `raven-reflect` crate.
    reflect_crate_path: Path,
}

impl<'a> ReflectMeta<'a> {
    pub fn new(type_name: &'a Ident, generics: &'a Generics, traits: ReflectTraits) -> Self {
        // let reflect_crate_path = CrateManifest::new(
        //         PathBuf::from("F://ILLmewWork//ProgrammingRelative//Raven-Engine//crates//bin//sandbox//Cargo.toml")
        //     ).get_path(RAVEN_REFLECT_CRATE_NAME);

        let reflect_crate_path = CrateManifest::get_path_default(RAVEN_REFLECT_CRATE_NAME);
        Self {
            type_name,
            generics,
            traits,
            reflect_crate_path,
        }
    }

    pub fn type_name(&self) -> &'a Ident {
        self.type_name
    }

    pub fn generics(&self) -> &'a Generics {
        self.generics
    }

    pub fn traits(&self) -> &ReflectTraits {
        &self.traits
    }

    pub fn reflect_crate_path(&self) -> &Path {
        &self.reflect_crate_path
    }

    /// Returns the `GetTypeRegistration` impl as a `TokenStream`.
    pub fn get_type_registration(&self) -> proc_macro2::TokenStream {
        reflect_gen::gen_type_registration(
            self.type_name,
            &self.reflect_crate_path,
            self.traits.idents(),
            self.generics,
            None,
        )
    }
}

impl<'a> ReflectTypeMetaInfo<'a> {
    pub fn from_derive_input(input: &'a DeriveInput) -> anyhow::Result<Self, syn::Error> {
        let mut traits = ReflectTraits::default();
        let mut reflect_mode = None;

        // check attributes
        for meta in input.attrs.iter().filter_map(|attr| attr.parse_meta().ok()) {
            match meta {
                // a meta list is like the derive(Copy) in #[derive(Copy)].
                Meta::List(meta_list) if meta_list.path.is_ident(REFLECT_ATTR) => {
                    if !matches!(reflect_mode, None | Some(ReflectMode::Common)) {
                        return Err(syn::Error::new(
                            meta_list.span(),
                            format_args!("Cannot use both `#[{REFLECT_ATTR}]` and `#[{REFLECT_PRIM_ATTR}]`"),
                        ));
                    }

                    reflect_mode = Some(ReflectMode::Common);

                    let new_trait = ReflectTraits::from_nested_meta(&meta_list.nested)?;
                    traits = traits.merge(new_trait)?;
                }
                Meta::List(meta_list) if meta_list.path.is_ident(REFLECT_PRIM_ATTR) => {
                    if !matches!(reflect_mode, None | Some(ReflectMode::Primitive)) {
                        return Err(syn::Error::new(
                            meta_list.span(),
                            format_args!("Cannot use both `#[{REFLECT_ATTR}]` and `#[{REFLECT_PRIM_ATTR}]`"),
                        ));
                    }

                    reflect_mode = Some(ReflectMode::Primitive);

                    let new_trait = ReflectTraits::from_nested_meta(&meta_list.nested)?;
                    traits = traits.merge(new_trait)?;
                }
                // a meta path is like the test in #[test].
                Meta::Path(meta_path) if meta_path.is_ident(REFLECT_PRIM_ATTR) => {
                    if !matches!(reflect_mode, None | Some(ReflectMode::Primitive)) {
                        return Err(syn::Error::new(
                            meta_path.span(),
                            format_args!("Cannot use both `#[{REFLECT_ATTR}]` and `#[{REFLECT_PRIM_ATTR}]`"),
                        ));
                    }

                    reflect_mode = Some(ReflectMode::Primitive);
                }
                _ => continue,
            }
        }

        let meta = ReflectMeta::new(&input.ident, &input.generics, traits);

        // default using ReflectMode::Common
        let reflect_mode = reflect_mode.unwrap_or(ReflectMode::Common);

        if reflect_mode == ReflectMode::Primitive {
            return Ok(Self::Primitive(meta));
        }

        match &input.data {
            Data::Struct(data) => {
                let struct_fields = Self::collect_struct_fields(&data.fields)?;

                let struct_meta = StructMetaInfo {
                    meta,
                    serialization_denylist: field_attributes::ignore_behaviors_to_serialization_deny_lists(
                        struct_fields.iter().map(|field| field.attrs.ignore_behavior)
                    ),
                    fields: struct_fields,
                };

                match data.fields {
                    Fields::Named(..) => Ok(Self::Struct(struct_meta)),
                    Fields::Unnamed(..) => Ok(Self::TupleStruct(struct_meta)),
                    Fields::Unit => Ok(Self::UnitStruct(struct_meta)),
                }
            }
            Data::Enum(data) => {
                let variants = Self::collect_enum_variants(&data.variants)?;

                let reflect_enum = EnumMetaInfo { meta, variants };
                Ok(Self::Enum(reflect_enum))
            }
            Data::Union(..) => Err(syn::Error::new(
                input.span(),
                "Reflection not supported for unions",
            ))
        }
    }

    fn collect_struct_fields(fields: &Fields) -> anyhow::Result<Vec<StructField>, syn::Error> {
        let struct_fields: ResultFlattener<StructField, SynError> = fields.iter().enumerate()
            .map(|(index, field)| -> anyhow::Result<StructField, SynError> {
                let attrs = field_attributes::parse_field_attributes(&field.attrs)?;

                Ok(StructField {
                    field,
                    attrs,
                    index,
                })
            })
            .fold(
                ResultFlattener::default(),
                ResultFlattener::fold
            );

        match struct_fields.finish() {
            Ok(res) => Ok(res),
            Err(inner) => Err(inner.inner)
        }
    }

    fn collect_enum_variants(variants: &Punctuated<Variant, Comma>) -> anyhow::Result<Vec<EnumVariant>, syn::Error> {
        let enum_variants: ResultFlattener<EnumVariant, SynError> = variants.iter().enumerate()
            .map(|(index, variant)| -> anyhow::Result<EnumVariant, SynError> {
                let fields = Self::collect_struct_fields(&variant.fields)?;

                let fields = match variant.fields {
                    Fields::Named(..) => EnumVariantFields::Named(fields),
                    Fields::Unnamed(..) => EnumVariantFields::Unnamed(fields),
                    Fields::Unit => EnumVariantFields::Unit,
                };

                Ok(EnumVariant {
                    variant,
                    attrs: field_attributes::parse_field_attributes(&variant.attrs)?,
                    fields,
                    index,
                })
            })
            .fold(
                ResultFlattener::default(),
                ResultFlattener::fold
            );
            
        match enum_variants.finish() {
            Ok(res) => Ok(res),
            Err(inner) => Err(inner.inner)
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ReflectMode {
    Common,
    Primitive,
}