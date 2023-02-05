use std::{any::{TypeId, Any}, slice::Iter};

use crate::{Reflect, DynamicTuple, UnnamedField};

/// Tuple struct that can be reflected at compile time.
pub trait Tuple: Reflect {
    /// Get a field of the tuple by index immutably.
    /// Return any type that can be reflected.
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;

    /// Get a field of the tuple by index mutably.
    /// Return any type that can be reflected.
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Return the number of fields.
    fn num_fields(&self) -> usize;

    /// Return an iterator to iterate over every reflected fields.
    fn iter(&self) -> TupleFieldIter;

    /// Drain the fields of this tuple to get a vector of owned values.
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>>;

    /// Clones the tuple into a [`DynamicTuple`].
    fn clone_dynamic(&self) -> DynamicTuple;
}

pub struct TupleFieldIter<'a> {
    pub(crate) refl_tuple: &'a dyn Tuple,
    pub(crate) curr_index: usize,
}

impl<'a> TupleFieldIter<'a> {
    pub fn new(value: &'a dyn Tuple) -> Self {
        Self {
            refl_tuple: value,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for TupleFieldIter<'a> {
    type Item = &'a dyn Reflect;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.refl_tuple.field_at(self.curr_index);
        self.curr_index += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Note: we already know the number of fields at compile-time reflected data.
        let field_len = self.refl_tuple.num_fields();
        (field_len, Some(field_len))
    }
}

impl<'a> ExactSizeIterator for TupleFieldIter<'a> {}

/// A container for compile-time tuple info.
#[derive(Clone, Debug)]
pub struct TupleTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
    fields: Box<[UnnamedField]>,
}

impl TupleTypeInfo {
    pub fn new<T: Reflect>(fields: &[UnnamedField]) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: fields.to_vec().into_boxed_slice(),
        }
    }

    pub fn field_at(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    pub fn iter(&self) -> Iter<'_, UnnamedField> {
        self.fields.iter()
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }
}