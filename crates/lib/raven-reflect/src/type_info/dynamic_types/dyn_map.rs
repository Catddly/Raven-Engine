use std::{collections::{HashMap, hash_map::Entry}, fmt::Formatter};

use crate::{Reflect, Map, MapIter, Typed, TypeInfo, NonGenericTypeInfoOnceCell, DynamicTypeInfo, special_traits::{partial_eq, debug}};

const HASH_ERROR: &'static str = "Given key in DynamicMap does not support hash!";

/// A map which allow key-values' type to be modified at runtime.
#[derive(Default)]
pub struct DynamicMap {
    name: String,
    kv_pairs: Vec<(Box<dyn Reflect>, Box<dyn Reflect>)>,
    /// Key Hash -> kv_pairs index
    indices: HashMap<u64, usize>,
}

impl DynamicMap {
    pub fn name(&self) -> &str {
        &self.name
    }
    
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    
    pub fn insert<K: Reflect, V: Reflect>(&mut self, key: K, value: V) {
        self.insert_boxed(Box::new(key), Box::new(value));
    }
}

// impl Typed
impl Typed for DynamicMap {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(
            DynamicTypeInfo::new::<Self>()
        ))
    }
}

// impl Debug
impl std::fmt::Debug for DynamicMap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

// impl Map
impl Map for DynamicMap {
    fn get(&self, key: &dyn Reflect) -> Option<&dyn Reflect> {
        self.indices
            .get(&key.reflect_hash().expect(HASH_ERROR))
            .cloned()
            .map(|index| &*self.kv_pairs.get(index).unwrap().1)
    }

    fn get_mut(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect> {
        self.indices
            .get(&key.reflect_hash().expect(HASH_ERROR))
            .cloned()
            .map(|index| &mut *self.kv_pairs.get_mut(index).unwrap().1)
    }

    fn get_at(&self, index: usize) -> Option<(&dyn Reflect, &dyn Reflect)> {
        self.kv_pairs
            .get(index)
            .map(|(k, v)| (&**k, &**v))
    }

    fn len(&self) -> usize {
        self.kv_pairs.len()
    }

    fn iter(&self) -> crate::MapIter {
        MapIter::new(self)
    }

    fn drain(self: Box<Self>) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)> {
        self.kv_pairs
    }

    fn clone_dynamic(&self) -> DynamicMap {
        DynamicMap {
            name: self.name.clone(),
            kv_pairs: self.kv_pairs
                .iter()
                .map(|(key, value)| (key.clone_value(), value.clone_value()))
                .collect(),
            indices: self.indices.clone(),
        }
    }

    fn insert_boxed(
        &mut self,
        key: Box<dyn Reflect>,
        mut value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>> {
        match self.indices.entry(key.reflect_hash().expect(HASH_ERROR)) {
            Entry::Occupied(entry) => {
                let (_old_key, old_value) = self.kv_pairs.get_mut(*entry.get()).unwrap();
                std::mem::swap(old_value, &mut value);
                Some(value)
            }
            Entry::Vacant(entry) => {
                entry.insert(self.kv_pairs.len());
                self.kv_pairs.push((key, value));
                None
            }
        }
    }

    fn remove(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        let index = self
            .indices
            .remove(&key.reflect_hash().expect(HASH_ERROR))?;
        let (_key, value) = self.kv_pairs.remove(index);
        Some(value)
    }
}

// impl Reflect
impl Reflect for DynamicMap {
    #[inline]
    fn type_name(&self) -> &'static str {
        ::core::any::type_name::<Self>()
    }

    #[inline]
    fn get_type_info(&self) -> &'static TypeInfo {
        <Self as Typed>::type_info()
    }
    
    #[inline]
    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> {
        self
    }

    #[inline]
    fn as_reflect(&self) -> &dyn Reflect {
        self
    }

    #[inline]
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
        self
    }

    #[inline]
    fn clone_value(&self) -> Box<dyn Reflect> {
        Box::new(self.clone_dynamic())
    }

    fn assign(&mut self, other: &dyn Reflect) {
        if let crate::ReflectRef::Map(map_value) = other.reflect_ref() {
            for (key, other) in map_value.iter() {
                if let Some(a_value) = self.get_mut(key) {
                    a_value.assign(other);
                } else {
                    self.insert_boxed(key.clone_value(), other.clone_value());
                }
            }
        } else {
            panic!("Attempted to apply a non-map type to a map type.");
        }
    }

    fn reflect_ref<'a>(&'a self) -> crate::ReflectRef<'a> {
        crate::ReflectRef::Map(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> crate::ReflectRefMut<'a> {
        crate::ReflectRefMut::Map(self)
    }

    fn reflect_owned<'a>(self: Box<Self>) -> crate::ReflectOwned {
        crate::ReflectOwned::Map(self)
    }

    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::map_partial_eq(self, value)
    }

    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicMap(")?;
        debug::map_debug(self, f)?;
        write!(f, ")")
    }
}