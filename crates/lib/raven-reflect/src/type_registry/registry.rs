use std::{collections::{HashMap, HashSet}, any::TypeId};

use crate::{Reflect, Typed};

use super::{TypeRegistration, GetTypeRegistration};

/// Registry for all reflected types.
pub struct TypeRegistry {
    registrations: HashMap<TypeId, TypeRegistration>,
    short_name_to_id: HashMap<String, TypeId>,
    full_name_to_id: HashMap<String, TypeId>,
    /// Type names on different crates might be the same,
    /// if we found collided short names, we use full name to remove ambiguity.
    ambiguous_names: HashSet<String>,
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
        // registry.register::<bool>();
        // registry.register::<u8>();
        // registry.register::<u16>();
        // registry.register::<u32>();
        // registry.register::<u64>();
        // registry.register::<u128>();
        // registry.register::<usize>();
        // registry.register::<i8>();
        // registry.register::<i16>();
        // registry.register::<i32>();
        // registry.register::<i64>();
        // registry.register::<i128>();
        // registry.register::<isize>();
        // registry.register::<f32>();
        // registry.register::<f64>();
        registry
    }

    pub fn register<T: GetTypeRegistration>(&mut self) {
        self.add_registration(T::get_type_registration());
    }

    pub fn add_registration(&mut self, registration: TypeRegistration) {
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
}