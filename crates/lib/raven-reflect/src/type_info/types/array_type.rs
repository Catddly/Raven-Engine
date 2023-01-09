use std::any::TypeId;

use crate::Reflect;

/// Compile-time known-sized array that can be reflected at compile time.
pub trait Array: Reflect {
    /// Return array element immutably by index, `None` if index is out of bounds.
    fn get(&self, index: usize) -> Option<&dyn Reflect>;

    /// Return array element mutably by index, `None` if index is out of bounds.
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Return the length of the array.
    fn len(&self) -> usize;

    /// Consume all elements and collect them into a [`Vec`].
    fn to_vec(self: Box<Self>) -> Vec<Box<dyn Reflect>>;

    /// Iterate over all elements in the array.
    fn iter(&self) -> ArrayIter;
}

pub struct ArrayIter<'a> {
    pub(crate) refl_array: &'a dyn Array,
    pub(crate) curr_index: usize,
}

impl<'a> ArrayIter<'a> {
    pub fn new(refl_array: &'a dyn Array) -> Self {
        Self {
            refl_array,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for ArrayIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.refl_array.get(self.curr_index);
        self.curr_index += 1;
        item
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.refl_array.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for ArrayIter<'a> {}

/// Storage container of array type.
#[derive(Debug, Clone)]
pub struct ArrayTypeInfo {
    type_name: &'static str,
    type_id: TypeId,
    item_type_name: &'static str,
    item_type_id: TypeId,
    capacity: usize,
}

impl ArrayTypeInfo {
    pub fn new<T: Array, I: Reflect>(capacity: usize) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            item_type_name: std::any::type_name::<I>(),
            item_type_id: TypeId::of::<I>(),
            capacity,
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

    pub fn item_type_name(&self) -> &'static str {
        self.item_type_name
    }

    pub fn item_type_id(&self) -> TypeId {
        self.item_type_id
    }

    /// Check if this field type matches the given type.
    pub fn item_is<T: std::any::Any>(&self) -> bool {
        TypeId::of::<T>() == self.item_type_id
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}