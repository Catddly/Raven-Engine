use std::any::TypeId;

use crate::Reflect;

/// Storage container of rust primitive type.
/// 
/// Primitive types can not be broken into other types further (i.e. it is not a compound type)
/// and rust provides this opaque type.
/// 
/// For example a [`bool`] can not be broken into other types further,
/// even though [`String`] can be broken into smaller types, but it is opaque to the user,
/// so we treat it as a primitive type. 
#[derive(Debug, Clone)]
pub struct PrimitiveTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
}

impl PrimitiveTypeInfo {
    pub fn new<T: Reflect + ?Sized>() -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is<T: std::any::Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }
}