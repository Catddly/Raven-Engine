use serde::de::{DeserializeSeed, Visitor, Error};

use crate::{EnumTypeInfo, VariantInfo, serde::visitors::ExpectedValues};

/// Deserializer to deserialize enum variant.
pub(super) struct VariantDeserializer {
    enum_info: &'static EnumTypeInfo,
}

impl VariantDeserializer {
    pub fn new(enum_info: &'static EnumTypeInfo) -> Self {
        Self {
            enum_info,
        }
    }
}

impl<'de> DeserializeSeed<'de> for VariantDeserializer {
    type Value = &'static VariantInfo;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct VariantVisitor(&'static EnumTypeInfo);

        impl<'de> Visitor<'de> for VariantVisitor {
            type Value = &'static VariantInfo;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Expected either a variant index or variant name")
            }

            fn visit_str<E>(self, variant_name: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.0.variant(variant_name).ok_or_else(|| {
                    let names = self.0.iter().map(|variant| variant.name());
                    Error::custom(format_args!(
                        "Unknown variant `{}`, expected one of {:?}",
                        variant_name,
                        ExpectedValues(names.collect())
                    ))
                })
            }

            fn visit_u32<E>(self, variant_index: u32) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.0.variant_at(variant_index as usize).ok_or_else(|| {
                    Error::custom(format_args!(
                        "No variant found at index `{}` on enum `{}`",
                        variant_index,
                        self.0.name()
                    ))
                })
            }
        }

        deserializer.deserialize_identifier(VariantVisitor(self.enum_info))
    }
}