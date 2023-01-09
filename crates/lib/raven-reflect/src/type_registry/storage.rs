use std::{collections::HashMap, any::TypeId, fmt::Debug};

use downcast_rs::{Downcast, impl_downcast};

use crate::{TypeInfo, Reflect, Typed, type_info_cell};

pub trait TypedMeta: Downcast + Send + Sync {
    fn clone_typed_meta(&self) -> Box<dyn TypedMeta>;
}
impl_downcast!(TypedMeta);

impl<T: 'static + Send + Sync> TypedMeta for T
where
    T: Clone,
{
    fn clone_typed_meta(&self) -> Box<dyn TypedMeta> {
        Box::new(self.clone())
    }
}

/// Container to hold the data of a type.
/// 
/// This can be used to downcast a `dyn Reflect` type into its origin concrete type.
pub struct TypeRegistration {
    pub(crate) short_name: String,
    pub(crate) typed_metadata: HashMap<TypeId, Box<dyn TypedMeta>>,
    /// type_info fetches from the Typed::type_info()
    pub(crate) type_info: &'static TypeInfo,
}

impl Debug for TypeRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // ignore typed_metadata
        f.debug_struct("TypeRegistration")
            .field("short_name", &self.short_name)
            .field("type_info", &self.type_info)
            .finish()
    }
}

impl Clone for TypeRegistration {
    fn clone(&self) -> Self {
        let mut typed_meta = HashMap::default();
        for (id, type_data) in &self.typed_metadata {
            typed_meta.insert(*id, (*type_data).clone_typed_meta());
        }

        Self {
            short_name: self.short_name.clone(),
            typed_metadata: typed_meta,
            type_info: self.type_info,
        }
    }
}

impl TypeRegistration {
    pub fn register<T: Reflect + Typed>() -> Self {
        let type_name = std::any::type_name::<T>();
        Self {
            typed_metadata: HashMap::default(),
            short_name: type_info_cell::get_type_collapsed_name(type_name),
            type_info: T::type_info(),
        }
    }

    /// Insert a new typed metadata for this type.
    /// 
    /// If metadata is already inserted, it will be replaced.
    pub fn insert<T: TypedMeta>(&mut self, data: T) {
        self.typed_metadata.insert(TypeId::of::<T>(), Box::new(data));
    }

    pub fn typed_meta<T: TypedMeta>(&self) -> Option<&T> {
        self.typed_metadata
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref())
    }

    pub fn typed_meta_mut<T: TypedMeta>(&mut self) -> Option<&mut T> {
        self.typed_metadata
            .get_mut(&TypeId::of::<T>())
            .and_then(|value| value.downcast_mut())
    }

    pub fn short_name(&self) -> &str {
        &self.short_name
    }

    pub fn type_name(&self) -> &'static str {
        self.type_info.type_name()
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_info.type_id()
    }

    pub fn type_info(&self) -> &'static TypeInfo {
        self.type_info
    }
}