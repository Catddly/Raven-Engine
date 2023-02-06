use std::{collections::HashMap, any::TypeId, fmt::Debug};

use downcast_rs::{Downcast, impl_downcast};

use crate::{TypeInfo, Reflect, Typed, type_info_cell, ptrs::{Ptr, PtrMut}};

use super::FromType;

/// Type meta information for certain reflected type.
pub trait TypeMeta: Downcast + Send + Sync {
    fn clone_typed_meta(&self) -> Box<dyn TypeMeta>;
}
impl_downcast!(TypeMeta);

impl<T: 'static + Send + Sync> TypeMeta for T
where
    T: Clone,
{
    fn clone_typed_meta(&self) -> Box<dyn TypeMeta> {
        Box::new(self.clone())
    }
}

/// Container to hold the data of a type.
/// 
/// This can be used to downcast a `dyn Reflect` type into its origin concrete type.
pub struct TypeRegistration {
    pub(crate) short_name: String,
    pub(crate) typed_metas: HashMap<TypeId, Box<dyn TypeMeta>>,
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
        for (id, type_data) in &self.typed_metas {
            typed_meta.insert(*id, (*type_data).clone_typed_meta());
        }

        Self {
            short_name: self.short_name.clone(),
            typed_metas: typed_meta,
            type_info: self.type_info,
        }
    }
}

impl TypeRegistration {
    pub fn type_of<T: Reflect + Typed>() -> Self {
        let type_name = std::any::type_name::<T>();
        Self {
            typed_metas: HashMap::default(),
            short_name: type_info_cell::get_type_collapsed_name(type_name),
            type_info: T::type_info(),
        }
    }

    /// Insert a new typed metadata for this type.
    /// 
    /// If metadata is already inserted, it will be replaced.
    pub fn insert<T: TypeMeta>(&mut self, data: T) {
        self.typed_metas.insert(TypeId::of::<T>(), Box::new(data));
    }

    pub fn type_meta<T: TypeMeta>(&self) -> Option<&T> {
        self.typed_metas
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref())
    }

    pub fn type_meta_mut<T: TypeMeta>(&mut self) -> Option<&mut T> {
        self.typed_metas
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

/// The reflected value at runtime always be type-erased (i.e. is a *const ()),
/// so in order to get its reflected data, we need to cast `*const () -> &dyn Reflect`.
/// 
/// This type will save function pointer to cast `*const () -> &dyn Reflect`,
/// and let user construct pointer into dyn Reflect.
#[derive(Clone)]
pub struct ReflectFromPtr {
    type_id: TypeId,
    to_reflect: for<'a> unsafe fn(Ptr<'a>) -> &'a dyn Reflect,
    to_reflect_mut: for<'a> unsafe fn(PtrMut<'a>) -> &'a mut dyn Reflect,
}

impl ReflectFromPtr {
    /// Returns the [`TypeId`] that the [`ReflectFromPtr`] was constructed for
    #[allow(dead_code)]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// # Safety
    ///
    /// `val` must be a pointer to value of the type that the [`ReflectFromPtr`] was constructed for.
    /// This can be verified by checking that the type id returned by [`ReflectFromPtr::type_id`] is the expected one.
    #[allow(dead_code)]
    pub unsafe fn as_reflect_ptr<'a>(&self, val: Ptr<'a>) -> &'a dyn Reflect {
        (self.to_reflect)(val)
    }

    /// # Safety
    ///
    /// `val` must be a pointer to a value of the type that the [`ReflectFromPtr`] was constructed for
    /// This can be verified by checking that the type id returned by [`ReflectFromPtr::type_id`] is the expected one.
    #[allow(dead_code)]
    pub unsafe fn as_reflect_ptr_mut<'a>(&self, val: PtrMut<'a>) -> &'a mut dyn Reflect {
        (self.to_reflect_mut)(val)
    }
}

impl<T: Reflect> FromType<T> for ReflectFromPtr {
    fn from_type() -> Self {
        ReflectFromPtr {
            type_id: std::any::TypeId::of::<T>(),
            to_reflect: |ptr| {
                // SAFE: only called from `as_reflect`, where the `ptr` is guaranteed to be of type `T`,
                // and `as_reflect_ptr`, where the caller promises to call it with type `T`
                unsafe { ptr.deref::<T>() as &dyn Reflect }
            },
            to_reflect_mut: |ptr| {
                // SAFE: only called from `as_reflect_mut`, where the `ptr` is guaranteed to be of type `T`,
                // and `as_reflect_ptr_mut`, where the caller promises to call it with type `T`
                unsafe { ptr.deref_mut::<T>() as &mut dyn Reflect }
            },
        }
    }
}