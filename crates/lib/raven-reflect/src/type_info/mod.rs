use std::any::TypeId;

mod types;

pub use types::*;

/// Type that has been reflected and can fetch type information.
pub trait Typed {
    fn type_info() -> &'static TypeInfo;
}

#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructTypeInfo),
    Array(ArrayTypeInfo),
    Primitive(PrimitiveTypeInfo),
}

impl TypeInfo {
    pub fn type_id(&self) -> TypeId {
        match &self {
            Self::Struct(ty) => ty.type_id(),
            Self::Array(ty) => ty.type_id(),
            Self::Primitive(ty) => ty.type_id(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match &self {
            Self::Struct(ty) => ty.type_name(),
            Self::Array(ty) => ty.type_name(),
            Self::Primitive(ty) => ty.type_name(),
        }
    }

    pub fn is<T: std::any::Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}