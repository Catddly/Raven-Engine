use std::{any::TypeId, slice::Iter};

use crate::{type_registry::{TypeRegistration, TypeRegistry}, type_info::{StructTypeInfo, NamedField}, UnnamedField, TupleVariantInfo, TupleTypeInfo, StructVariantInfo};

/// Any type can get field's type registration from type registry.
pub(super) trait GetFieldTypeRegistration<'a, E: serde::de::Error> {
    fn get_field_registration(
        &self,
        index: usize,
        registry: &'a TypeRegistry,
    ) -> Result<&'a TypeRegistration, E>;
}

impl<'a, E: serde::de::Error> GetFieldTypeRegistration<'a, E> for StructTypeInfo {
    fn get_field_registration(
        &self,
        index: usize,
        registry: &'a TypeRegistry,
    ) -> Result<&'a TypeRegistration, E> {
        let field = self.field_at(index).ok_or_else(|| {
            serde::de::Error::custom(format_args!(
                "No field at index {} on struct {}",
                index,
                self.type_name(),
            ))
        })?;
        get_registration(field.type_id(), field.type_name(), registry)
    }
}

impl<'a, E: serde::de::Error> GetFieldTypeRegistration<'a, E> for StructVariantInfo {
    fn get_field_registration(
        &self,
        index: usize,
        registry: &'a TypeRegistry,
    ) -> Result<&'a TypeRegistration, E> {
        let field = self.field_at(index).ok_or_else(|| {
            serde::de::Error::custom(format_args!(
                "No field at index {} on variant struct {}",
                index,
                self.name(),
            ))
        })?;
        get_registration(field.type_id(), field.type_name(), registry)
    }
}

pub(super) fn get_registration<'a, E: serde::de::Error>(
    type_id: TypeId,
    type_name: &str,
    registry: &'a TypeRegistry,
) -> Result<&'a TypeRegistration, E> {
    let registration = registry.registration(type_id).ok_or_else(|| {
        serde::de::Error::custom(format_args!(
            "no registration found for type `{}`",
            type_name
        ))
    })?;
    Ok(registration)
}

pub(super) trait StructLikeTypeInfo {
    fn get_name(&self) -> &str;
    fn get_field(&self, name: &str) -> Option<&NamedField>;
    fn iter(&self) -> Iter<'_, NamedField>;
}

impl StructLikeTypeInfo for StructTypeInfo {
    fn get_name(&self) -> &str {
        self.name()
    }

    fn get_field(&self, name: &str) -> Option<&NamedField> {
        self.field(name)
    }

    fn iter(&self) -> Iter<'_, NamedField> {
        self.iter()
    }
}

impl StructLikeTypeInfo for StructVariantInfo {
    fn get_name(&self) -> &str {
        self.name()
    }

    fn get_field(&self, name: &str) -> Option<&NamedField> {
        self.field(name)
    }

    fn iter(&self) -> Iter<'_, NamedField> {
        self.iter()
    }
}

pub(super) trait TupleLikeTypeInfo {
    fn get_name(&self) -> &str;
    fn get_field(&self, index: usize) -> Option<&UnnamedField>;
    fn num_fields(&self) -> usize;
}

impl TupleLikeTypeInfo for TupleTypeInfo {
    fn get_name(&self) -> &str {
        self.type_name()
    }

    fn get_field(&self, index: usize) -> Option<&UnnamedField> {
        self.field_at(index)
    }

    fn num_fields(&self) -> usize {
        self.num_fields()
    }
}

impl TupleLikeTypeInfo for TupleVariantInfo {
    fn get_name(&self) -> &str {
        self.name()
    }

    fn get_field(&self, index: usize) -> Option<&UnnamedField> {
        self.field_at(index)
    }

    fn num_fields(&self) -> usize {
        self.num_fields()
    }
}