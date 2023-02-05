use crate::{Reflect, type_registry::FromType};

/// Reflect traits to provide [`Default`] behavior to reflected types.
#[derive(Clone)]
pub struct ReflectDefault {
    default_func: fn() -> Box<dyn Reflect>,
}

impl ReflectDefault {
    pub fn default(&self) -> Box<dyn Reflect> {
        (self.default_func)()
    }
}

impl<T: Reflect + Default> FromType<T> for ReflectDefault {
    fn from_type() -> Self {
        ReflectDefault {
            default_func: || Box::<T>::default(),
        }
    }
}