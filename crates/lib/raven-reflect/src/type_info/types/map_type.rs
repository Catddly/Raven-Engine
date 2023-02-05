use std::any::{TypeId, Any};
use std::hash::Hash;

use crate::{Reflect, DynamicMap};

/// Any ordered map-like type which has key-value pairs.
/// 
/// This reflected map type may not be consistent between instances,
/// 'Map' entries is _NOT_ guaranteed to be stable across runs or between instances. 
pub trait Map: Reflect {
    fn get(&self, key: &dyn Reflect) -> Option<&dyn Reflect>;

    fn get_mut(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect>;

    fn get_at(&self, index: usize) -> Option<(&dyn Reflect, &dyn Reflect)>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter(&self) -> MapIter;

    fn drain(self: Box<Self>) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)>;

    /// Clones the map, producing a [`DynamicMap`].
    fn clone_dynamic(&self) -> DynamicMap;

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    /// If the map did have this key present, the value is updated, and the old value is returned.
    fn insert_boxed(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>>;

    /// Removes an entry from the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    /// If the map did have this key present, the removed value is returned.
    fn remove(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>>;
}

pub struct MapIter<'a> {
    pub(crate) refl_map: &'a dyn Map,
    pub(crate) curr_index: usize,
}

impl<'a> MapIter<'a> {
    pub fn new(refl_map: &'a dyn Map) -> Self {
        Self {
            refl_map,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for MapIter<'a> {
    type Item = (&'a dyn Reflect, &'a dyn Reflect);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.refl_map.get_at(self.curr_index);
        self.curr_index += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let field_len = self.refl_map.len();
        (field_len, Some(field_len))
    }
}

impl<'a> ExactSizeIterator for MapIter<'a> {}

/// Storage container of map-like type.
#[derive(Clone, Debug)]
pub struct MapTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
    key_type_name: &'static str,
    key_type_id: TypeId,
    value_type_name: &'static str,
    value_type_id: TypeId,
}

impl MapTypeInfo {
    /// Create a new [`MapInfo`].
    pub fn new<T: Map, K: Hash + Reflect, V: Reflect>() -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            key_type_name: std::any::type_name::<K>(),
            key_type_id: TypeId::of::<K>(),
            value_type_name: std::any::type_name::<V>(),
            value_type_id: TypeId::of::<V>(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Check if the given type matches the map type.
    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }

    pub fn key_type_name(&self) -> &'static str {
        self.key_type_name
    }

    pub fn key_type_id(&self) -> TypeId {
        self.key_type_id
    }

    pub fn key_is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.key_type_id
    }

    pub fn value_type_name(&self) -> &'static str {
        self.value_type_name
    }

    pub fn value_type_id(&self) -> TypeId {
        self.value_type_id
    }

    pub fn value_is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.value_type_id
    }
}