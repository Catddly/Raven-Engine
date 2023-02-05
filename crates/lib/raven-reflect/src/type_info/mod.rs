use std::any::TypeId;

mod types;
mod dynamic_types;

pub use types::*;
pub use dynamic_types::*;

/// Type that has been reflected and can fetch type information.
pub trait Typed {
    fn type_info() -> &'static TypeInfo;
}

#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructTypeInfo),
    TupleStruct(TupleStructTypeInfo),
    Tuple(TupleTypeInfo),
    Enum(EnumTypeInfo),
    Array(ArrayTypeInfo),
    List(ListTypeInfo),
    Map(MapTypeInfo),
    Primitive(PrimitiveTypeInfo),
    Dynamic(DynamicTypeInfo)
}

impl TypeInfo {
    pub fn type_id(&self) -> TypeId {
        match &self {
            Self::Struct(ty) => ty.type_id(),
            Self::TupleStruct(ty) => ty.type_id(),
            Self::Tuple(ty) => ty.type_id(),
            Self::Enum(ty) => ty.type_id(),
            Self::Array(ty) => ty.type_id(),
            Self::List(ty) => ty.type_id(),
            Self::Map(ty) => ty.type_id(),
            Self::Primitive(ty) => ty.type_id(),
            Self::Dynamic(ty) => ty.type_id(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match &self {
            Self::Struct(ty) => ty.type_name(),
            Self::TupleStruct(ty) => ty.type_name(),
            Self::Tuple(ty) => ty.type_name(),
            Self::Enum(ty) => ty.type_name(),
            Self::Array(ty) => ty.type_name(),
            Self::List(ty) => ty.type_name(),
            Self::Map(ty) => ty.type_name(),
            Self::Primitive(ty) => ty.type_name(),
            Self::Dynamic(ty) => ty.type_name(),
        }
    }

    pub fn is<T: std::any::Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
}