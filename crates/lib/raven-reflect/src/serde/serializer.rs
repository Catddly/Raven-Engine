use serde::{
    Serialize,
    ser::{
        SerializeMap, SerializeStruct, SerializeStructVariant,
        SerializeTupleVariant, Error, SerializeTupleStruct, SerializeTuple, SerializeSeq
    }
};

use crate::{Reflect, type_registry::TypeRegistry, ReflectRef, type_info::{Struct}, TypeInfo, Enum, VariantForm, VariantInfo, TupleStruct, Tuple, Map, List, Array};

use super::reflect_ser::{SerializationData, self};

fn get_type_info<E: serde::ser::Error>(
    type_info: &'static TypeInfo,
    type_name: &str,
    registry: &TypeRegistry,
) -> Result<&'static TypeInfo, E> {
    match type_info {
        TypeInfo::Dynamic(..) => match registry.registration_with_full_name(type_name) {
            Some(registration) => Ok(registration.type_info()),
            None => Err(Error::custom(format_args!(
                "no registration found for dynamic type with name {}",
                type_name
            ))),
        },
        info => Ok(info),
    }
}

/// Serializer to serialize any reflected type into key-value pairs.
/// 
/// Key: full type name
/// Value: serialization value of that type
pub struct ReflectSerializer<'a> {
    pub reflected: &'a dyn Reflect,
    pub registry: &'a TypeRegistry,
}

impl<'a> ReflectSerializer<'a> {
    pub fn new(reflected: &'a dyn Reflect, registry: &'a TypeRegistry) -> Self {
        Self {
            reflected,
            registry,
        }
    }
}

impl<'a> Serialize for ReflectSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(
            self.reflected.type_name(),
            &TypedReflectSerializer::new(self.reflected, self.registry)
        )?;
        map.end()
    }
}

/// Serializer for types which is known and required no additional metadata to serialize this type.
pub struct TypedReflectSerializer<'a> {
    pub reflected: &'a dyn Reflect,
    pub registry: &'a TypeRegistry,
}

impl<'a> TypedReflectSerializer<'a> {
    pub fn new(reflected: &'a dyn Reflect, registry: &'a TypeRegistry) -> Self {
        Self {
            reflected,
            registry,
        }
    }
}

impl<'a> Serialize for TypedReflectSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        let serializable = reflect_ser::get_reflect_serializable(
            self.reflected,
            self.registry
        );

        if let Ok(serializable) = serializable {
            // if this type have ReflectSerialize (i.e. have #[reflect(Reflect)])
            return serializable.borrow().serialize(serializer);
        }

        match self.reflected.reflect_ref() {
            ReflectRef::Struct(value) => StructSerializer {
                reflected_struct: value,
                registry: self.registry,
            }
            .serialize(serializer),
            ReflectRef::Enum(value) => EnumSerializer {
                reflected_enum: value,
                registry: self.registry,
            }
            .serialize(serializer),
            ReflectRef::Tuple(value) => TupleSerializer {
                tuple: value,
                registry: self.registry,
            }.serialize(serializer),
            ReflectRef::TupleStruct(value) => TupleStructSerializer {
                tuple_struct: value,
                registry: self.registry,
            }.serialize(serializer),
            ReflectRef::Array(value) => ArraySerializer {
                array: value,
                registry: self.registry,
            }.serialize(serializer),
            ReflectRef::List(value) => ListSerializer {
                list: value,
                registry: self.registry,
            }.serialize(serializer),
            ReflectRef::Map(value) => MapSerializer {
                map: value,
                registry: self.registry,
            }.serialize(serializer),
            ReflectRef::Primitive(_) => Err(serializable.err().unwrap()),
        }
    }
}

/// Serializer to serialize a reflected struct.
pub struct StructSerializer<'a> {
    pub reflected_struct: &'a dyn Struct,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for StructSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_info = get_type_info(
            self.reflected_struct.get_type_info(),
            self.reflected_struct.type_name(),
            self.registry,
        )?;

        let struct_info = match type_info {
            TypeInfo::Struct(struct_info) => struct_info,
            info => {
                return Err(serde::ser::Error::custom(format_args!(
                    "Expected struct type but received {:?}",
                    info
                )));
            }
        };

        let serialization_data = self
            .registry
            .registration(type_info.type_id())
            .and_then(|registration| registration.type_meta::<SerializationData>());
        let num_ignore_fields = serialization_data.map(|data| data.num_ignore_fields()).unwrap_or(0);

        let mut state = serializer.serialize_struct(
            struct_info.name(),
            self.reflected_struct.num_fields() - num_ignore_fields,
        )?;

        for (index, reflected) in self.reflected_struct.iter().enumerate() {
            if serialization_data
                .map(|data| data.is_ignore_field(index))
                .unwrap_or(false)
            {
                continue;
            }

            let key = struct_info.field_at(index).unwrap().name();
            state.serialize_field(key, &TypedReflectSerializer::new(reflected, self.registry))?;
        }

        state.end()
    }
}

/// Serializer to serialize a reflected enum.
pub struct EnumSerializer<'a> {
    pub reflected_enum: &'a dyn Enum,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for EnumSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_info = get_type_info(
            self.reflected_enum.get_type_info(),
            self.reflected_enum.type_name(),
            self.registry,
        )?;

        let enum_info = match type_info {
            TypeInfo::Enum(enum_info) => enum_info,
            info => {
                return Err(serde::ser::Error::custom(format_args!(
                    "Expected enum type but received {:?}",
                    info
                )));
            }
        };

        let enum_name = enum_info.name();
        let variant_index = self.reflected_enum.variant_index() as u32;
        let variant_info = enum_info
            .variant_at(variant_index as usize)
            .ok_or_else(|| {
                serde::ser::Error::custom(format_args!(
                    "Variant at index `{}` does not exist",
                    variant_index
                ))
            })?;
        let variant_name = variant_info.name();
        let variant_form = self.reflected_enum.variant_form();
        let num_field = self.reflected_enum.num_fields();

        match variant_form {
            VariantForm::Unit => {
                // differentiate with Option
                if self.reflected_enum
                    .type_name()
                    .starts_with("core::option::Option")
                {
                    serializer.serialize_none()
                } else {
                    serializer.serialize_unit_variant(enum_name, variant_index, variant_name)
                }
            }
            VariantForm::Struct => {
                let struct_info = match variant_info {
                    VariantInfo::Struct(struct_info) => struct_info,
                    info => {
                        return Err(serde::ser::Error::custom(format_args!(
                            "Expected struct variant form but received {:?}",
                            info
                        )));
                    }
                };

                let mut state = serializer.serialize_struct_variant(
                    enum_name,
                    variant_index,
                    variant_name,
                    num_field,
                )?;
                for (index, field) in self.reflected_enum.iter().enumerate() {
                    let field_info = struct_info.field_at(index).unwrap();
                    state.serialize_field(
                        field_info.name(),
                        &TypedReflectSerializer::new(field.value(), self.registry),
                    )?;
                }
                state.end()
            }
            VariantForm::Tuple if num_field == 1 => {
                let field = self.reflected_enum.field_at(0).unwrap();
                // differentiate with Option
                if self.reflected_enum
                    .type_name()
                    .starts_with("core::option::Option")
                {
                    serializer.serialize_some(&TypedReflectSerializer::new(field, self.registry))
                } else {
                    serializer.serialize_newtype_variant(
                        enum_name,
                        variant_index,
                        variant_name,
                        &TypedReflectSerializer::new(field, self.registry),
                    )
                }
            }
            VariantForm::Tuple => {
                let mut state = serializer.serialize_tuple_variant(
                    enum_name,
                    variant_index,
                    variant_name,
                    num_field,
                )?;
                for field in self.reflected_enum.iter() {
                    state.serialize_field(&TypedReflectSerializer::new(
                        field.value(),
                        self.registry,
                    ))?;
                }
                state.end()
            }
        }
    }
}

/// Serializer to serialize a reflected tuple struct.
pub struct TupleStructSerializer<'a> {
    pub tuple_struct: &'a dyn TupleStruct,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for TupleStructSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_info = get_type_info(
            self.tuple_struct.get_type_info(),
            self.tuple_struct.type_name(),
            self.registry,
        )?;

        let tuple_struct_info = match type_info {
            TypeInfo::TupleStruct(tuple_struct_info) => tuple_struct_info,
            info => {
                return Err(Error::custom(format_args!(
                    "Expected tuple struct type but received {:?}",
                    info
                )));
            }
        };

        let serialization_data = self
            .registry
            .registration(type_info.type_id())
            .and_then(|registration| registration.type_meta::<SerializationData>());

        let ignored_len = serialization_data.map(|data| data.num_ignore_fields()).unwrap_or(0);
        let mut state = serializer.serialize_tuple_struct(
            tuple_struct_info.name(),
            self.tuple_struct.num_fields() - ignored_len,
        )?;

        for (index, value) in self.tuple_struct.iter().enumerate() {
            if serialization_data
                .map(|data| data.is_ignore_field(index))
                .unwrap_or(false)
            {
                continue;
            }
            state.serialize_field(&TypedReflectSerializer::new(value, self.registry))?;
        }
        state.end()
    }
}

pub struct TupleSerializer<'a> {
    pub tuple: &'a dyn Tuple,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for TupleSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_tuple(self.tuple.num_fields())?;

        for value in self.tuple.iter() {
            state.serialize_element(&TypedReflectSerializer::new(value, self.registry))?;
        }
        state.end()
    }
}

pub struct MapSerializer<'a> {
    pub map: &'a dyn Map,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for MapSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_map(Some(self.map.len()))?;
        for (key, value) in self.map.iter() {
            state.serialize_entry(
                &TypedReflectSerializer::new(key, self.registry),
                &TypedReflectSerializer::new(value, self.registry),
            )?;
        }
        state.end()
    }
}

pub struct ListSerializer<'a> {
    pub list: &'a dyn List,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for ListSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_seq(Some(self.list.len()))?;
        for value in self.list.iter() {
            state.serialize_element(&TypedReflectSerializer::new(value, self.registry))?;
        }
        state.end()
    }
}

pub struct ArraySerializer<'a> {
    pub array: &'a dyn Array,
    pub registry: &'a TypeRegistry,
}

impl<'a> Serialize for ArraySerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_tuple(self.array.len())?;
        for value in self.array.iter() {
            state.serialize_element(&TypedReflectSerializer::new(value, self.registry))?;
        }
        state.end()
    }
}