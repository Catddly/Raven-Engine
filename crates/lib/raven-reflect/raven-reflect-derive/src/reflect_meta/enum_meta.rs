use proc_macro2::Ident;
use syn::Variant;
use quote::quote;

use crate::{field_attributes::ReflectFieldAttr};

use super::{ReflectMeta, struct_meta::StructField};

pub(crate) struct EnumVariant<'a> {
    pub variant: &'a Variant,
    pub fields: EnumVariantFields<'a>,
    #[allow(dead_code)]
    pub attrs: ReflectFieldAttr,
    #[allow(dead_code)]
    pub index: usize,
}

pub(crate) enum EnumVariantFields<'a> {
    Named(Vec<StructField<'a>>),
    Unnamed(Vec<StructField<'a>>),
    Unit,
}

pub(crate) struct EnumMetaInfo<'a> {
    pub(super) meta: ReflectMeta<'a>,
    pub(super) variants: Vec<EnumVariant<'a>>,
}

impl<'a> EnumMetaInfo<'a> {
    pub fn meta(&self) -> &ReflectMeta<'a> {
        &self.meta
    }

    /// Returns the given ident as a qualified unit variant of this enum.
    pub fn get_enum_unit(&self, variant: &Ident) -> proc_macro2::TokenStream {
        let name = self.meta.type_name;
        // e.g. Foo::Hello
        quote! {
            #name::#variant
        }
    }

    /// The complete set of variants in this enum.
    pub fn variants(&self) -> &[EnumVariant<'a>] {
        &self.variants
    }
}