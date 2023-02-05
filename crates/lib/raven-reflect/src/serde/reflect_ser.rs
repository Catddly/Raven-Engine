use std::collections::HashSet;

use crate::{Reflect, type_registry::{FromType, TypeRegistry}};

/// Object safe dyn Serialize wrapper.
/// 
/// Use erased_serde::Serialize to achieve object safe.
pub enum ReflectSerializable<'a> {
    Owned(Box<dyn erased_serde::Serialize + 'a>),
    Borrowed(&'a dyn erased_serde::Serialize),
}

impl<'a> ReflectSerializable<'a> {
    pub fn borrow(&self) -> &dyn erased_serde::Serialize {
        match self {
            ReflectSerializable::Owned(serialize) => serialize,
            ReflectSerializable::Borrowed(serialize) => serialize,
        }
    }
}

pub(crate) fn get_reflect_serializable<'a, E: serde::ser::Error>(
    reflected: &'a dyn Reflect,
    type_registry: &TypeRegistry,
) -> Result<ReflectSerializable<'a>, E> {
    let reflect_serialize = type_registry
        .type_meta::<ReflectSerialize>(reflected.type_id())
        .ok_or_else(|| {
            serde::ser::Error::custom(format_args!(
                "Type '{}' did not register type meta ReflectSerialize",
                reflected.type_name()
            ))
        })?;
    Ok(reflect_serialize.get_reflect_serializable(reflected))
}

/// Serializer for give instance of reflected type.
/// 
/// Used as a functor class to get ReflectSerializable and serialize it.
#[derive(Clone)]
pub struct ReflectSerialize {
    get_reflect_serializable: for<'a> fn(&'a dyn Reflect) -> ReflectSerializable<'a>,
}

impl<T: Reflect + erased_serde::Serialize> FromType<T> for ReflectSerialize {
    fn from_type() -> Self {
        ReflectSerialize {
            get_reflect_serializable: |value| {
                let value = value.downcast_ref::<T>().unwrap_or_else(|| {
                    panic!("ReflectSerialize::get_serialize called with type `{}`, even though it was created for `{}`", value.type_name(), std::any::type_name::<T>())
                });
                ReflectSerializable::Borrowed(value)
            },
        }
    }
}

impl ReflectSerialize {
    /// Turn the reflected into a serializable representation.
    pub fn get_reflect_serializable<'a>(&self, reflected: &'a dyn Reflect) -> ReflectSerializable<'a> {
        (self.get_reflect_serializable)(reflected)
    }
}

#[derive(Debug, Clone)]
pub struct SerializationData {
    ignore_indices: HashSet<usize>,
}

impl SerializationData {
    pub fn new<I: Iterator<Item = usize>>(ignore_indices_iter: I) -> Self {
        Self {
            ignore_indices: ignore_indices_iter.collect()
        }
    }

    pub fn is_ignore_field(&self, index: usize) -> bool {
        self.ignore_indices.contains(&index)
    }

    pub fn num_ignore_fields(&self) -> usize {
        self.ignore_indices.len()
    }
}