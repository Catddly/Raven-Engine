use std::any::TypeId;

use crate::Reflect;

/// Storage container of any dynamic type.
#[derive(Clone, Debug)]
pub struct DynamicTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
}

impl DynamicTypeInfo {
    pub fn new<T: Reflect>() -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            #[cfg(feature = "documentation")]
            docs: None,
        }
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
