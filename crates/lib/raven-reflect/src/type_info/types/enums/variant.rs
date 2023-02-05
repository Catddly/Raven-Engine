use std::{collections::HashMap, slice::Iter};

use crate::type_info::{NamedField, UnnamedField};

/// Enum variant forms.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum VariantForm {
    /// ```
    /// enum Foo {
    ///     Hello {
    ///         field_a: f32,
    ///     },
    /// }
    /// ```
    Struct,
    /// ```
    /// enum Foo {
    ///     Hello(f32),
    /// }
    /// ```
    Tuple,
    /// ```
    /// enum Foo {
    ///     Hello,
    /// }
    /// ```
    Unit
}

/// Information about the variant of enum.
#[derive(Debug, Clone)]
pub enum VariantInfo {
    Struct(StructVariantInfo),
    Tuple(TupleVariantInfo),
    Unit(UnitVariantInfo),
}

impl VariantInfo {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Struct(info) => info.name(),
            Self::Tuple(info) => info.name(),
            Self::Unit(info) => info.name(),
        }
    }
}

/// Struct form of the enum variant.
/// It is similar to the [`StructTypeInfo`].
/// It has no type_name nor type_id since it is an anonymous struct.
/// 
/// [`StructTypeInfo`]: crate::type_info::StructTypeInfo
#[derive(Debug, Clone)]
pub struct StructVariantInfo {
    name: &'static str,
    fields: Box<[NamedField]>,
    field_names: Box<[&'static str]>,
    field_indices: HashMap<&'static str, usize>,
}

impl StructVariantInfo {
    pub fn new(struct_name: &'static str, fields: &[NamedField]) -> Self {
        let field_names = fields.iter()
           .map(|field| field.name())
           .collect();

        let field_indices = fields.iter().enumerate()
            .map(|(idx, field)| (field.name(), idx))
            .collect();

        Self {
            name: struct_name,
            fields: fields.to_vec().into_boxed_slice(),
            field_names,
            field_indices,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn field_at(&self, index: usize) -> Option<&NamedField> {
        self.fields.get(index)
    }

    pub fn field(&self, name: &str) -> Option<&NamedField> {
        self.field_indices.get(name)
           .and_then(|idx| self.fields.get(*idx))
    }

    pub fn field_names(&self) -> &[&'static str] {
        &self.field_names
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    pub fn index_of(&self, field_name: &'static str) -> Option<usize> {
        self.field_indices.get(field_name).copied()
    }

    pub fn iter(&self) -> Iter<'_, NamedField> {
        self.fields.iter()
    }
}

/// Tuple form of the enum variant.
#[derive(Debug, Clone)]
pub struct TupleVariantInfo {
    name: &'static str,
    fields: Box<[UnnamedField]>,
}

impl TupleVariantInfo {
    pub fn new(name: &'static str, fields: &[UnnamedField]) -> Self {
        Self {
            name,
            fields: fields.to_vec().into_boxed_slice(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn field_at(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    pub fn iter(&self) -> Iter<'_, UnnamedField> {
        self.fields.iter()
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }
}

/// Unit form of the enum variant.
#[derive(Debug, Clone)]
pub struct UnitVariantInfo {
    name: &'static str,
}

impl UnitVariantInfo {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            #[cfg(feature = "documentation")]
            docs: None,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}
