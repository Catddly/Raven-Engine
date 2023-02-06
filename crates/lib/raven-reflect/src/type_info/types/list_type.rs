use std::any::{TypeId, Any};

use crate::{Reflect, FromReflect, DynamicList};

use super::array_type::Array;

/// An random access runtime array type, corresponding to [`Vec`] in Rust.
/// 
/// This is just a extension to [`Array`] trait to have ability to _push_ and _pop_
/// element at runtime and change the length of the array.
pub trait List: Reflect + Array {
    /// Inserts an element at position `index` within the list,
    /// shifting all elements after it towards the back of the list.
    ///
    /// # Panics
    /// Panics if `index > len`.
    fn insert(&mut self, index: usize, element: Box<dyn Reflect>);

    /// Removes and returns the element at position `index` within the list,
    /// shifting all elements before it towards the front of the list.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    fn remove(&mut self, index: usize) -> Box<dyn Reflect>;

    fn push(&mut self, value: Box<dyn Reflect>);

    fn pop(&mut self) -> Option<Box<dyn Reflect>>;

    /// Clones the list, producing a [`DynamicList`].
    fn clone_dynamic(&self) -> DynamicList {
        DynamicList::new(
            self.type_name().to_string(),
            self.iter().map(|value| value.clone_value()).collect()
        )
    }
}

/// Storage container of list type.
/// (list is a runtime array)
#[derive(Clone, Debug)]
pub struct ListTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
    item_type_name: &'static str,
    item_type_id: TypeId,
}

impl ListTypeInfo {
    pub fn new<T: List, Item: FromReflect>() -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            item_type_name: std::any::type_name::<Item>(),
            item_type_id: TypeId::of::<Item>(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }

    pub fn item_type_name(&self) -> &'static str {
        self.item_type_name
    }

    pub fn item_type_id(&self) -> TypeId {
        self.item_type_id
    }

    pub fn item_is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.item_type_id
    }
}