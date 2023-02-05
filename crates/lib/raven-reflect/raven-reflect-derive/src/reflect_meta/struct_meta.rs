use crate::{field_attributes::ReflectFieldAttr, reflect_gen};

use super::ReflectMeta;

use syn::{Field};
use bit_set::BitSet;

pub(crate) struct StructField<'a> {
    pub field: &'a Field,
    /// Reflection attributes.
    pub attrs: ReflectFieldAttr,
    /// Index of this field.
    pub index: usize,
}

pub(crate) struct StructMetaInfo<'a> {
    pub(super) meta: ReflectMeta<'a>,
    pub(super) serialization_denylist: BitSet<u32>,
    pub(super) fields: Vec<StructField<'a>>,
}

impl<'a> StructMetaInfo<'a> {
    pub fn meta(&self) -> &ReflectMeta {
        &self.meta
    }

    #[allow(dead_code)]
    pub fn serialization_denylist(&self) -> &BitSet<u32> {
        &self.serialization_denylist
    }

    #[allow(dead_code)]
    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    /// Returns the `GetTypeRegistration` impl as a `TokenStream`.
    ///
    /// Returns a specific implementation for structs and this method should be preferred over the generic [`get_type_registration`](crate::ReflectMeta) method.
    pub fn get_type_registration(&self) -> proc_macro2::TokenStream {
        let reflect_crate_path = self.meta.reflect_crate_path();

        reflect_gen::gen_type_registration(
            self.meta.type_name(),
            reflect_crate_path,
            self.meta.traits().idents(),
            self.meta.generics(),
            Some(&self.serialization_denylist),
        )
    }

    /// Get a collection of types of opaque fields.
    pub fn opaque_types(&self) -> Vec<syn::Type> {
        self.fields.iter()
            .filter(move |field| field.attrs.ignore_behavior.is_opaque())
            .map(|field| field.field.ty.clone())
            .collect()
    }

    /// Get an iterator on a collection of opaque fields.
    pub fn opaque_fields(&self) -> impl Iterator<Item = &StructField<'a>> {
        self.fields.iter()
            .filter(move |field| field.attrs.ignore_behavior.is_opaque())
    }

    /// Get an iterator on collection of transparent fields.
    pub fn transparent_fields(&self) -> impl Iterator<Item = &StructField<'a>> {
        self.fields.iter()
            .filter(move |field| field.attrs.ignore_behavior.is_transparent())
    }
}