use crate::{Reflect, type_registry::FromType};

/// Arbitrary type can convert from Reflect to its exact type.
pub trait FromReflect: Reflect + Sized {
    /// Construct a concrete instance from a dyn Reflect.
    /// Return None if the type check fails.
    fn from_reflect(reflected: &dyn Reflect) -> Option<Self>;

    /// Attempts to downcast the given value to `Self` using,
    /// constructing the value using [`from_reflect`] if that fails.
    ///
    /// This method is more efficient than using [`from_reflect`] for cases where
    /// the given value is likely a boxed instance of `Self` (i.e. `Box<Self>`)
    /// rather than a boxed dynamic type (e.g. [`DynamicStruct`], [`DynamicList`], etc.).
    ///
    /// [`from_reflect`]: Self::from_reflect
    /// [`DynamicStruct`]: crate::DynamicStruct
    /// [`DynamicList`]: crate::DynamicList
    fn take_from_reflect(reflect: Box<dyn Reflect>) -> Result<Self, Box<dyn Reflect>> {
        match reflect.take::<Self>() {
            Ok(value) => Ok(value),
            Err(value) => match Self::from_reflect(value.as_ref()) {
                None => Err(value),
                Some(value) => Ok(value),
            },
        }
    }
}

/// Convert function container of trait FromReflect.
/// 
/// It stores a function to convert arbitrary reflected instance (Box<dyn Reflect> or &dyn Reflect)
/// to a concrete instance.
/// 
/// It is useful when you have type which is _not_ known at compile time,
/// you can construct a concrete from a dynamic type.
/// 
/// (e.g. construct a `MyStruct` from `DynamicStruct` with the same fields, and using `ReflectFromReflect` from the
/// [`TypeRegistry`] to convert it to `MyStruct`)
#[derive(Clone)]
pub struct ReflectFromReflect {
    from_reflect_func: fn(&dyn Reflect) -> Option<Box<dyn Reflect>>,
}

impl ReflectFromReflect {
    #[allow(clippy::wrong_self_convention)]
    pub fn from_reflect(&self, reflected: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        (self.from_reflect_func)(reflected)
    }
}

impl<T: FromReflect> FromType<T> for ReflectFromReflect {
    fn from_type() -> Self {
        Self {
            from_reflect_func: |reflected| {
                T::from_reflect(reflected).map(|value| Box::new(value) as Box<dyn Reflect>)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{self as raven_reflect, FromReflect, DynamicStruct};
    use raven_reflect_derive::{Reflect, FromReflect};

    #[test]
    fn test_from_reflect_struct() {
        #[derive(Reflect, FromReflect, Debug, Eq, PartialEq)]
        struct TestStruct {
            #[reflect(default)]
            a: String,
            #[reflect(default = "get_b_default")]
            b: u32,
            #[reflect(default = "get_c_default")]
            #[reflect(transparent)]
            c: i32,
        }

        fn get_b_default() -> u32 {
            58
        }

        fn get_c_default() -> i32 {
            -85
        }

        let test = TestStruct {
            a: String::from(""),
            b: 58,
            c: -85,
        };

        let dyn_struct = DynamicStruct::default();
        let from_reflect = <TestStruct as FromReflect>::from_reflect(&dyn_struct).unwrap();

        assert_eq!(from_reflect, test);
    }
}