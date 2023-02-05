use std::any::TypeId;

use crate::Reflect;

/// Named filed of a reflected struct.
#[derive(Clone, Debug)]
pub struct NamedField {
    /// Field name shown in the code.
    name: &'static str,
    /// Field type's name which fetches from [`std::any::Any`].
    type_name: &'static str,
    /// Type id from the [`TypeId`].
    type_id: TypeId,
}

impl NamedField {
    pub fn new<T: Reflect>(field_name: &'static str) -> Self {
        Self {
            name: field_name,
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
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
}

/// Unnamed filed of a reflected tuple or tuple struct.
#[derive(Clone, Debug)]
pub struct UnnamedField {
    /// Field type's name which fetches from ['std::any::Any'].
    type_name: &'static str,
    /// Type id from the [`TypeId`].
    type_id: TypeId,
    /// Subscript index of the tuple.
    index: usize,
}

impl UnnamedField {
    pub fn new<T: Reflect>(index: usize) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            index,
        }
    }

    pub fn index(&self) -> usize {
        self.index
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
}

/// Convenience helper trait for user to get the field data of a struct.
/// 
/// GetField will do fetching and downcasting for you.
pub trait GetField {
    /// Get a field of struct by name immutably.
    fn get_field<R: Reflect>(&self, name: &str) -> Option<&R>;
    
    /// Get a field of struct by name mutably.
    fn get_field_mut<R: Reflect>(&mut self, name: &str) -> Option<&mut R>;
}