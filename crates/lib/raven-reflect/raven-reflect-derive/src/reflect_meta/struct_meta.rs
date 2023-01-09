use crate::field_attributes::ReflectFieldAttr;

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

    pub fn serialization_denylist(&self) -> &BitSet<u32> {
        &self.serialization_denylist
    }

    pub fn fields(&self) -> &[StructField] {
        &self.fields
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