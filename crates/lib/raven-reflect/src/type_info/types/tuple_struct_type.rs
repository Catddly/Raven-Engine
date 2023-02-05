use std::{any::TypeId, slice::Iter};

use crate::{UnnamedField, Reflect, DynamicTupleStruct};

/// Tuple struct type in Rust, which has anonymous fields.
pub trait TupleStruct: Reflect {
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;

    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    fn num_fields(&self) -> usize;

    fn iter(&self) -> TupleStructFieldIter;

    fn clone_dynamic(&self) -> DynamicTupleStruct;
}

pub struct TupleStructFieldIter<'a> {
    pub(crate) refl_tuple_struct: &'a dyn TupleStruct,
    pub(crate) curr_index: usize,
}

impl<'a> TupleStructFieldIter<'a> {
    pub fn new(value: &'a dyn TupleStruct) -> Self {
        Self {
            refl_tuple_struct: value,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for TupleStructFieldIter<'a> {
    type Item = &'a dyn Reflect;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.refl_tuple_struct.field_at(self.curr_index);
        self.curr_index += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Note: we already know the number of fields at compile-time reflected data.
        let field_len = self.refl_tuple_struct.num_fields();
        (field_len, Some(field_len))
    }
}

impl<'a> ExactSizeIterator for TupleStructFieldIter<'a> {}

/// Storage container of tuple struct type.
#[derive(Clone, Debug)]
pub struct TupleStructTypeInfo {
    name: &'static str,
    type_name: &'static str,
    type_id: TypeId,
    fields: Box<[UnnamedField]>,
}

impl TupleStructTypeInfo {
    pub fn new<T: Reflect>(struct_name: &'static str, fields: &[UnnamedField]) -> Self {
        Self {
            name: struct_name,
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: fields.to_vec().into_boxed_slice(),
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

    pub fn field_at(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    pub fn iter(&self) -> Iter<'_, UnnamedField> {
        self.fields.iter()
    }
}