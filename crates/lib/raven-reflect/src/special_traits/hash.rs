use std::hash::{Hash, Hasher};

use crate::{Array, Enum};

/// Returns the `u64` hash of the given [enum](Enum).
#[inline]
pub fn enum_hash<T: Enum>(value: &T) -> Option<u64> {
    let mut hasher = crate::ReflectHasher::default();

    std::any::Any::type_id(value).hash(&mut hasher);
    value.variant_name().hash(&mut hasher);
    value.variant_form().hash(&mut hasher);
    for field in value.iter() {
        hasher.write_u64(field.value().reflect_hash()?);
    }
    
    Some(hasher.finish())
}

/// Returns the `u64` hash of the given [array](Array).
#[inline]
pub fn array_hash<A: Array>(array: &A) -> Option<u64> {
    let mut hasher = crate::ReflectHasher::default();
    
    std::any::Any::type_id(array).hash(&mut hasher);
    array.len().hash(&mut hasher);
    for value in array.iter() {
        hasher.write_u64(value.reflect_hash()?);
    }

    Some(hasher.finish())
}