use serde::Deserialize;

use crate::{Reflect, type_registry::FromType};

#[derive(Clone)]
pub struct ReflectDeserialize {
    pub deserialize_func: fn(
        deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<Box<dyn Reflect>, erased_serde::Error>,
}

impl ReflectDeserialize {
    pub fn deserialize<'de, D>(&self, deserializer: D) -> Result<Box<dyn Reflect>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // into trait object
        let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);

        (self.deserialize_func)(&mut erased)
            .map_err(<<D as serde::Deserializer<'de>>::Error as serde::de::Error>::custom)
    }
}

impl<T: for<'a> Deserialize<'a> + Reflect> FromType<T> for ReflectDeserialize {
    fn from_type() -> Self {
        ReflectDeserialize {
            deserialize_func: |deserializer| Ok(Box::new(T::deserialize(deserializer)?)),
        }
    }
}