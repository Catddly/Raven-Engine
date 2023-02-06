use std::{collections::{HashMap, HashSet}, any::TypeId};

use crate::{Reflect};

use super::{TypeRegistration, GetTypeRegistration, TypeMeta, FromType};

/// Registry for all reflected types.
pub struct TypeRegistry {
    registrations: HashMap<TypeId, TypeRegistration>,
    short_name_to_id: HashMap<String, TypeId>,
    full_name_to_id: HashMap<String, TypeId>,
    /// Type names on different crates might be the same,
    /// if we found collided short names, we use full name to remove ambiguity.
    ambiguous_names: HashSet<String>,
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeRegistry {
    /// Create a new empty type registry.
    pub fn empty() -> Self {
        Self {
            registrations: Default::default(),
            short_name_to_id: Default::default(),
            full_name_to_id: Default::default(),
            ambiguous_names: Default::default(),
        }
    }

    /// Create a type registry with default registrations for primitive types.
    pub fn new() -> Self {
        let mut registry = Self::empty();
        registry.register::<char>();
        registry.register::<bool>();
        registry.register::<u8>();
        registry.register::<u16>();
        registry.register::<u32>();
        registry.register::<u64>();
        registry.register::<u128>();
        registry.register::<usize>();
        registry.register::<i8>();
        registry.register::<i16>();
        registry.register::<i32>();
        registry.register::<i64>();
        registry.register::<i128>();
        registry.register::<isize>();
        registry.register::<f32>();
        registry.register::<f64>();
        registry
    }

    pub fn register<T: GetTypeRegistration>(&mut self) {
        self.add_registration(T::get_type_registration());
    }

    fn add_registration(&mut self, registration: TypeRegistration) {
        if self.registrations.contains_key(&registration.type_id()) {
            return;
        }
            
        let short_name = registration.short_name.to_string();
        if self.short_name_to_id.contains_key(&short_name)
            || self.ambiguous_names.contains(&short_name)
        {
            // name is ambiguous. fall back to long names for all ambiguous types
            self.short_name_to_id.remove(&short_name);
            self.ambiguous_names.insert(short_name);
        } else {
            self.short_name_to_id.insert(short_name, registration.type_id());
        }

        self.full_name_to_id.insert(registration.type_name().to_string(), registration.type_id());
        self.registrations.insert(registration.type_id(), registration);
    }

    /// Register typed metadata for certain reflecte type.
    /// 
    /// # Example
    /// 
    /// ```ignore
    /// registry.register::<T>();
    /// /// Type T can be reflected and serialized.
    /// registry.register_typed_meta::<T, ReflectSerialize>();
    /// /// Type T can be reflected and deserialized.
    /// registry.register_typed_meta::<T, ReflectDeserialize>();
    /// /// All typed meta datas are store in TypeRegistration.
    /// ```
    pub fn register_type_meta<T: Reflect + 'static, D: TypeMeta + FromType<T>>(&mut self) {
        let data = self.registration_mut(TypeId::of::<T>()).unwrap_or_else(|| {
            panic!(
                "Attempted to call `TypeRegistry::register_type_data` for type `{T}` with data `{D}` without registering `{T}` first",
                T = std::any::type_name::<T>(),
                D = std::any::type_name::<D>(),
            )
        });
        data.insert(D::from_type());
    }

    /// Get type registration for certain type id immutably.
    pub fn registration(&self, type_id: TypeId) -> Option<&TypeRegistration> {
        self.registrations.get(&type_id)
    }

    /// Get type registration for certain type id mutably.
    pub fn registration_mut(&mut self, type_id: TypeId) -> Option<&mut TypeRegistration> {
        self.registrations.get_mut(&type_id)
    }

    /// Return Some() type's registration by its full type name,
    /// None() if this type doesn't exist. 
    pub fn registration_with_full_name(&self, type_name: &str) -> Option<&TypeRegistration> {
        self.full_name_to_id
            .get(type_name)
            .and_then(|id| self.registration(*id))
    }

    /// Return Some() type's registration by its full type name,
    /// None() if this type doesn't exist. 
    pub fn registration_with_full_name_mut(&mut self, type_name: &str) -> Option<&mut TypeRegistration> {
        self.full_name_to_id
            .get(type_name)
            .cloned()
            .and_then(move |id| self.registration_mut(id))
    }

    /// Return Some() type's registration by its full type name,
    /// None() if this type doesn't exist.
    /// Notice that ambiguous short type name will also return None.
    pub fn registration_with_short_name(&self, type_name: &str) -> Option<&TypeRegistration> {
        self.short_name_to_id
            .get(type_name)
            .and_then(|id| self.registration(*id))
    }

    /// Return Some() type's registration by its full type name,
    /// None() if this type doesn't exist.
    /// Notice that ambiguous short type name will also return None.
    pub fn registration_with_short_name_mut(&mut self, type_name: &str) -> Option<&mut TypeRegistration> {
        self.short_name_to_id
            .get(type_name)
            .cloned()
            .and_then(move |id| self.registration_mut(id))
    }

    /// Return Some() typed meta by its type id,
    /// None() if this type doesn't exist.
    pub fn type_meta<D: TypeMeta>(&self, type_id: TypeId) -> Option<&D> {
        self.registration(type_id)
            .and_then(|registration| registration.type_meta::<D>())
    }

    /// Return Some() typed meta by its type id,
    /// None() if this type doesn't exist.
    pub fn type_meta_mut<D: TypeMeta>(&mut self, type_id: TypeId) -> Option<&mut D> {
        self.registration_mut(type_id)
            .and_then(|registration| registration.type_meta_mut::<D>())
    }
}