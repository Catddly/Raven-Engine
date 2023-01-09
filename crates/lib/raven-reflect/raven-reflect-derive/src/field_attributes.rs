use bit_set::BitSet;
use quote::ToTokens;
use syn::{Attribute, Meta, NestedMeta};

use crate::REFLECT_ATTR;

pub(crate) static IGNORE_SERIALIZATION_ATTR: &str = "no_serialization";
pub(crate) static IGNORE_ALL_ATTR: &str = "transparent";

/// Enum to define field should be ignore for serialization or reflection.
/// 
/// Notice that a field must be reflected before it can be serialized, we ensure this relationship
/// by this enum.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReflectIgnoreBehavior {
    /// Ignore nothing.
    #[default]
    None,
    /// Ignore only the serialization of this field.
    IgnoreSerialization,
    /// Ignore both the serialization and reflection.
    IgnoreAll,
}

impl ReflectIgnoreBehavior {
    /// Return if it is reflect transparent.
    /// Reflect transparent means this object is transparent to the reflection metadata generator,
    /// so it will _NOT_ be reflected, generator treats it transparent (i.e. doesn't exist)
    pub fn is_transparent(&self) -> bool {
        match &self {
            Self::IgnoreAll => true,
            Self::None | Self::IgnoreSerialization => false,
        }
    }

    pub fn is_opaque(&self) -> bool {
        !self.is_transparent()
    }
}

pub(crate) fn ignore_behaviors_to_serialization_deny_lists<T: Iterator<Item = ReflectIgnoreBehavior>>(iter: T) -> BitSet<u32> {
    let mut bitset = BitSet::default();

    iter.fold(0, |next_idx, member| match member {
        // just skip the transparent member
        ReflectIgnoreBehavior::IgnoreAll => next_idx,
        ReflectIgnoreBehavior::IgnoreSerialization => {
            bitset.insert(next_idx);
            next_idx + 1
        }
        ReflectIgnoreBehavior::None => next_idx + 1,
    });

    bitset
}

/// container for attributes defined on a reflected type's field.
#[derive(Default)]
pub(crate) struct ReflectFieldAttr {
    pub ignore_behavior: ReflectIgnoreBehavior,
}

pub(crate) fn parse_field_attributes(attrs: &[Attribute]) -> anyhow::Result<ReflectFieldAttr, syn::Error> {
    let mut res = ReflectFieldAttr::default();
    let mut errors: Option<syn::Error> = None;

    // we only care about the `reflect` attributes.
    let attr_iter = attrs.iter()
        .filter(|attr| attr.path.is_ident(REFLECT_ATTR));

    for attr in attr_iter {
        let attr_meta = attr.parse_meta()?;
        let parse_res = parse_attribute_meta(&mut res, &attr_meta);
        if let Err(error) = parse_res {
            // combine all errors while parsing one field
            if let Some(errors) = &mut errors {
                errors.combine(error)
            } else {
                errors = Some(error);
            }
        }
    }

    if let Some(errors) = errors {
        Err(errors)
    } else {
        Ok(res)
    }
}

fn parse_attribute_meta(reflect_attr: &mut ReflectFieldAttr, meta: &Meta) -> anyhow::Result<(), syn::Error> {
    match meta {
        // a meta path is like the test in #[test].
        Meta::Path(path) if path.is_ident(IGNORE_SERIALIZATION_ATTR) => {
            (reflect_attr.ignore_behavior == ReflectIgnoreBehavior::None)
                .then(|| reflect_attr.ignore_behavior = ReflectIgnoreBehavior::IgnoreSerialization)
                .ok_or_else(|| syn::Error::new_spanned(path, format!("Only one of ['{IGNORE_SERIALIZATION_ATTR}','{IGNORE_ALL_ATTR}'] is allowed")))
        }
        Meta::Path(path) if path.is_ident(IGNORE_ALL_ATTR) => {
            (reflect_attr.ignore_behavior == ReflectIgnoreBehavior::None)
                .then(|| reflect_attr.ignore_behavior = ReflectIgnoreBehavior::IgnoreAll)
                .ok_or_else(|| syn::Error::new_spanned(path, format!("Only one of ['{IGNORE_SERIALIZATION_ATTR}','{IGNORE_ALL_ATTR}'] is allowed")))
        }
        Meta::Path(path) => Err(
            syn::Error::new_spanned(path, format!("Unknown reflect attributes: {}", path.to_token_stream()))
        ),
        // a name-value meta is like the path = "..." in #[path = "sys/windows.rs"].
        Meta::NameValue(named) => Err(
            syn::Error::new_spanned(named, format!("Unexpected named attributes: {}", named.to_token_stream()))
        ),
        // a meta list is like the derive(Copy) in #[derive(Copy)].
        Meta::List(list) if !list.path.is_ident(REFLECT_ATTR) => Err(
            syn::Error::new_spanned(list, "Unexpected property!")
        ),
        Meta::List(list) => {
            for nested in &list.nested {
                if let NestedMeta::Meta(meta) = nested {
                    // recursively parse Meta::List
                    parse_attribute_meta(reflect_attr, meta)?;
                }
            }
            Ok(())
        }
    }
}