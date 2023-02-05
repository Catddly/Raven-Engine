use serde::{de::{DeserializeSeed, Visitor, Error}};

use crate::{
    type_registry::{TypeRegistry, TypeRegistration},
    Reflect, TypeInfo,
};

use super::{
    reflect_de::ReflectDeserialize,
    visitors::{OptionVisitor, StructVisitor, EnumVisitor, TupleStructVisitor, TupleVisitor, ListVisitor, MapVisitor, ArrayVisitor},
};

/// A reflected type deserializer used when you don't know the type ahead of time.
/// (i.e. the type will be registered at runtime)
/// 
/// Deserializer will deserialize value into Box<dyn Reflect>.
/// If the type is non primitive type, it will be converted into its corresponding dynamic type.
/// (e.g. Struct -> DynamicStruct)
/// If the type is primitive type, it will contained the exact value.
/// (e.g. f32 -> copy of f32)
/// 
/// This means that converting to any concrete instance will require the use of
/// [`FromReflect`], or downcasting for value types.
/// 
/// Because the type isn't known ahead of time, the serialized data must take the form of
/// a map containing the following entries (in order):
/// 1. `type`: The _full_ [type name]
/// 2. `value`: The serialized value of the reflected type
/// 
/// If the type is already known and the [`TypeInfo`] for it can be retrieved,
/// [`TypedReflectDeserializer`] may be used instead to avoid requiring these entries.
pub struct UntypedReflectDeserializer<'a> {
    registry: &'a TypeRegistry,
}

impl<'a> UntypedReflectDeserializer<'a> {
    pub fn new(registry: &'a TypeRegistry) -> Self {
        Self {
            registry
        }
    }
}

// we need to carry type registry as a context to deserialize reflected data
impl<'a, 'de> DeserializeSeed<'de> for UntypedReflectDeserializer<'a> {
    type Value = Box<dyn Reflect>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_map(UntypedReflectDeserializerVisitor {
            registry: self.registry,
        })
    }
}

struct UntypedReflectDeserializerVisitor<'a> {
    registry: &'a TypeRegistry,
}

impl<'a, 'de> Visitor<'de> for UntypedReflectDeserializerVisitor<'a> {
    type Value = Box<dyn Reflect>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("Map containing `type` and `value` entries for the reflected value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>
    {
        // retrieve entry
        let full_type_name = map
            .next_key::<String>()?
            .ok_or_else(|| Error::invalid_length(0, &"At least one entry"))?;
        let registration = self.registry
            .registration_with_full_name(&full_type_name)
            .ok_or_else(|| { Error::custom(format_args!("No registration found for `{full_type_name}`"))
        })?;

        let value = map.next_value_seed(TypedReflectDeserializer {
            registration,
            registry: self.registry,
        })?;
        Ok(value)
    }
}

/// A reflected type deserializer used when you know the type ahead of time.
/// (i.e. you know its TypeInfo)
/// 
/// Deserializer will deserialize value into Box<dyn Reflect>.
/// If the type is non primitive type, it will be converted into its corresponding dynamic type.
/// (e.g. Struct -> DynamicStruct)
/// If the type is primitive type, it will contained the exact value.
/// (e.g. f32 -> copy of f32)
/// 
/// This means that converting to any concrete instance will require the use of
/// [`FromReflect`], or downcasting for value types.
/// 
/// If the type is not known ahead of time, use [`UntypedReflectDeserializer`] instead.
pub struct TypedReflectDeserializer<'a> {
    registration: &'a TypeRegistration,
    registry: &'a TypeRegistry,
}

impl<'a> TypedReflectDeserializer<'a> {
    pub fn new(registration: &'a TypeRegistration, registry: &'a TypeRegistry) -> Self {
        Self {
            registration,
            registry,
        }
    }
}

impl<'a, 'de> DeserializeSeed<'de> for TypedReflectDeserializer<'a> {
    type Value = Box<dyn Reflect>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        let type_name = self.registration.type_name();

        // handle both Primitive case and types that have a custom `ReflectDeserialize`
        if let Some(deserialize_reflect) = self.registration.type_meta::<ReflectDeserialize>() {
            let value = deserialize_reflect.deserialize(deserializer)?;
            return Ok(value);
        }

        match self.registration.type_info() {
            TypeInfo::Struct(ty_info) => {
                let mut dynamic_struct = deserializer.deserialize_struct(
                    ty_info.name(),
                    ty_info.field_names(),
                    StructVisitor::new(
                        ty_info,
                        self.registration,
                        self.registry,
                    )
                )?;
                dynamic_struct.set_name(type_name.to_string());
                Ok(Box::new(dynamic_struct))
            }
            TypeInfo::TupleStruct(ty_info) => {
                let mut dynamic_tuple_struct = deserializer.deserialize_tuple_struct(
                    ty_info.name(),
                    ty_info.num_fields(),
                    TupleStructVisitor::new(
                        ty_info,
                        self.registry,
                        self.registration,
                    ),
                )?;
                dynamic_tuple_struct.set_name(ty_info.type_name().to_string());
                Ok(Box::new(dynamic_tuple_struct))
            }
            TypeInfo::Tuple(ty_info) => {
                let mut dynamic_tuple = deserializer.deserialize_tuple(
                    ty_info.num_fields(),
                    TupleVisitor::new(
                        ty_info,
                        self.registry,
                    ),
                )?;
                dynamic_tuple.set_name(ty_info.type_name().to_string());
                Ok(Box::new(dynamic_tuple))
            }
            TypeInfo::Enum(ty_info) => {
                let type_name = ty_info.type_name();
                let mut dynamic_enum = if type_name.starts_with("core::option::Option") {
                    deserializer.deserialize_option(OptionVisitor::new(
                        ty_info,
                        self.registry,
                    ))?
                } else {
                    deserializer.deserialize_enum(
                        ty_info.name(),
                        ty_info.variant_names(),
                        EnumVisitor::new(
                            ty_info,
                            self.registration,
                            self.registry,
                        ),
                    )?
                };
                dynamic_enum.set_name(type_name.to_string());
                Ok(Box::new(dynamic_enum))
            }
            TypeInfo::List(ty_info) => {
                let mut dynamic_list = deserializer.deserialize_seq(
                    ListVisitor::new(
                        ty_info,
                        self.registry,
                    ),
                )?;
                dynamic_list.set_name(ty_info.type_name().to_string());
                Ok(Box::new(dynamic_list))
            }
            TypeInfo::Map(ty_info) => {
                let mut dynamic_map = deserializer.deserialize_seq(
                    MapVisitor::new(
                        ty_info,
                        self.registry,
                    ),
                )?;
                dynamic_map.set_name(ty_info.type_name().to_string());
                Ok(Box::new(dynamic_map))
            }
            TypeInfo::Array(ty_info) => {
                let mut dynamic_array  = deserializer.deserialize_seq(
                    ArrayVisitor::new(
                        ty_info,
                        self.registry,
                    ),
                )?;
                dynamic_array.set_name(ty_info.type_name().to_string());
                Ok(Box::new(dynamic_array))
            }
            TypeInfo::Primitive(_) => {
                // this case should already be handled
                Err(Error::custom(format_args!(
                    "The TypeRegistration for {} doesn't have ReflectDeserialize",
                    type_name
                )))
            }
            _ => unimplemented!(),
        }
    }
}