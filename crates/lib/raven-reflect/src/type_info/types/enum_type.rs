use std::{any::TypeId, collections::HashMap, slice::Iter};

use crate::{Reflect, DynamicEnum};

use super::enums::{VariantForm, VariantFieldIter, VariantInfo};

pub trait Enum: Reflect {
    fn field(&self, name: &str) -> Option<&dyn Reflect>;
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect>;
    
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    fn index_of(&self, name: &str) -> Option<usize>;

    fn field_name_at(&self, index: usize) -> Option<&str>;

    fn iter(&self) -> VariantFieldIter;

    fn num_fields(&self) -> usize;
    
    /// Return the name of current variant.
    fn variant_name(&self) -> &str;
    /// Return the index of current variant.
    fn variant_index(&self) -> usize;
    /// Return the form of current variant.
    /// To gain more infos see: ['VariantForm'] 
    fn variant_form(&self) -> VariantForm;

    fn clone_dynamic(&self) -> DynamicEnum;

    /// Returns true if the current variant's form matches the given one.
    fn is_form(&self, variant_form: VariantForm) -> bool {
        self.variant_form() == variant_form
    }
    
    /// Returns the full path to the current variant.
    fn variant_path(&self) -> String {
        format!("{}::{}", self.type_name(), self.variant_name())
    }
}

/// Storage container of struct type.
/// 
/// Enum is implemented using union in Rust, different with Cpp.
#[derive(Debug, Clone)]
pub struct EnumTypeInfo {
    name: &'static str,
    type_name: &'static str,
    type_id: TypeId,
    variants: Box<[VariantInfo]>,
    variant_names: Box<[&'static str]>,
    variant_indices: HashMap<&'static str, usize>,
}

impl EnumTypeInfo {
    pub fn new<T: Reflect>(enum_name: &'static str, variants: &[VariantInfo]) -> Self {
        let variant_names = variants.iter()
           .map(|field| field.name())
           .collect();

        let variant_indices = variants.iter().enumerate()
            .map(|(idx, field)| (field.name(), idx))
            .collect();

        Self {
            name: enum_name,
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            variants: variants.to_vec().into_boxed_slice(),
            variant_names,
            variant_indices,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Check if this field type matches the given type.
    pub fn is<T: std::any::Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }

    /// Return a slice of names of all variants.
    pub fn variant_names(&self) -> &[&'static str] {
        &self.variant_names
    }

    pub fn variant(&self, name: &str) -> Option<&VariantInfo> {
        self.variant_indices
            .get(name)
            .map(|index| &self.variants[*index])
    }

    pub fn variant_at(&self, index: usize) -> Option<&VariantInfo> {
        self.variants.get(index)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.variant_indices.get(name).copied()
    }

    pub fn variant_path(&self, name: &str) -> String {
        format!("{}::{name}", self.type_name())
    }

    pub fn contains_variant(&self, name: &str) -> bool {
        self.variant_indices.contains_key(name)
    }

    pub fn iter(&self) -> Iter<'_, VariantInfo> {
        self.variants.iter()
    }

    pub fn num_variants(&self) -> usize {
        self.variants.len()
    }
}