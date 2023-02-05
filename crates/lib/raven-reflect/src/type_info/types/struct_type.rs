use std::{any::TypeId, collections::HashMap, slice::Iter};

use crate::{Reflect, DynamicStruct};

use super::field::{NamedField, GetField};

/// Struct and unit struct that can be reflected at compile time.
pub trait Struct: Reflect {
    /// Get a field of the struct by name immutably.
    /// Return any type that can be reflected.
    fn field(&self, name: &str) -> Option<&dyn Reflect>;

    /// Get a field of the struct by name mutably.
    /// Return any type that can be reflected.
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect>;

    /// Get a field of the struct by index immutably.
    /// Return any type that can be reflected.
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;

    /// Get a field of the struct by index mutably.
    /// Return any type that can be reflected.
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Return the number of fields.
    fn num_fields(&self) -> usize;

    /// Get the name of field by index.
    fn field_name_at(&self, index: usize) -> Option<&str>;

    /// Return an iterator to iterate over every reflected fields.
    fn iter(&self) -> StructFieldIter;

    /// Clones the struct into a [`DynamicStruct`].
    fn clone_dynamic(&self) -> DynamicStruct;
}

pub struct StructFieldIter<'a> {
    pub(crate) refl_struct: &'a dyn Struct,
    pub(crate) curr_index: usize,
}

impl<'a> StructFieldIter<'a> {
    pub fn new(refl_struct: &'a dyn Struct) -> Self {
        Self {
            refl_struct,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for StructFieldIter<'a> {
    type Item = &'a dyn Reflect;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.refl_struct.field_at(self.curr_index);
        self.curr_index += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Note: we already know the number of fields at compile-time reflected data.
        let field_len = self.refl_struct.num_fields();
        (field_len, Some(field_len))
    }
}

impl<T: Struct> GetField for T {
    fn get_field<R: Reflect>(&self, name: &str) -> Option<&R> {
        self.field(name)
            .and_then(|field| field.downcast_ref::<R>())
    }

    fn get_field_mut<R: Reflect>(&mut self, name: &str) -> Option<&mut R> {
        self.field_mut(name)
            .and_then(|field| field.downcast_mut::<R>())    
    }
}

impl GetField for dyn Struct {
    fn get_field<R: Reflect>(&self, name: &str) -> Option<&R> {
        self.field(name)
            .and_then(|field| field.downcast_ref::<R>())
    }

    fn get_field_mut<R: Reflect>(&mut self, name: &str) -> Option<&mut R> {
        self.field_mut(name)
            .and_then(|field| field.downcast_mut::<R>())    
    }
}

/// Storage container of struct type.
#[derive(Debug, Clone)]
pub struct StructTypeInfo {
    /// Name of the struct.
    name: &'static str,
    /// Type name of the struct from [`std::any::Any`].
    type_name: &'static str,
    type_id: TypeId,
    /// Runtime heap allocated fixed-size array of fields.
    fields: Box<[NamedField]>,
    /// Runtime heap allocated fixed-size array of field' name.
    field_names: Box<[&'static str]>,
    /// For fast backward search.
    field_indices: HashMap<&'static str, usize>,
}

impl StructTypeInfo {
    pub fn new<T: Reflect>(struct_name: &'static str, fields: &[NamedField]) -> Self {
        let field_names = fields.iter()
           .map(|field| field.name())
           .collect();

        let field_indices = fields.iter().enumerate()
            .map(|(idx, field)| (field.name(), idx))
            .collect();

        Self {
            name: struct_name,
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: fields.to_vec().into_boxed_slice(),
            field_names,
            field_indices,
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

    pub fn field_at(&self, index: usize) -> Option<&NamedField> {
        self.fields.get(index)
    }

    pub fn field(&self, name: &str) -> Option<&NamedField> {
        self.field_indices.get(name)
           .and_then(|idx| self.fields.get(*idx))
    }

    pub fn field_names(&self) -> &[&'static str] {
        &self.field_names
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    pub fn index_of(&self, field_name: &'static str) -> Option<usize> {
        self.field_indices.get(field_name).copied()
    }

    pub fn iter(&self) -> Iter<'_, NamedField> {
        self.fields.iter()
    }
}