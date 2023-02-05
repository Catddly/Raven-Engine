use std::fmt::Display;

use serde::{de::{Visitor, Error, MapAccess, SeqAccess, DeserializeSeed, VariantAccess}, Deserialize};

use crate::{
    EnumTypeInfo,
    TupleStruct,
    type_registry::{TypeRegistry, TypeRegistration},
    DynamicEnum, VariantInfo, TypedReflectDeserializer,
    DynamicTuple, DynamicStruct, TupleVariantInfo, SerializationData,
    StructVariantInfo, DynamicVariant, StructTypeInfo, TupleStructTypeInfo, DynamicTupleStruct, TupleTypeInfo, ListTypeInfo, DynamicList, MapTypeInfo, DynamicMap, Map, ArrayTypeInfo, DynamicArray
};

use super::{
    field_ty_registration::{get_registration, StructLikeTypeInfo, self, TupleLikeTypeInfo, GetFieldTypeRegistration},
    variant_deserializer::VariantDeserializer
};

pub(super) struct StructVisitor<'a> {
    ty_info: &'static StructTypeInfo,
    registration: &'a TypeRegistration,
    registry: &'a TypeRegistry,
}

impl<'a> StructVisitor<'a> {
    pub fn new(
        ty_info: &'static StructTypeInfo,
        registration: &'a TypeRegistration,
        registry: &'a TypeRegistry
    ) -> Self { 
        Self { ty_info, registration, registry } 
    }
}

impl<'a, 'de> Visitor<'de> for StructVisitor<'a> {
    type Value = DynamicStruct;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected struct value")
    }

    // a struct type info is a key-value pair
    // where key is the full type name and value is the fields 
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>
    {
        visit_struct(&mut map, self.ty_info, self.registry)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>
    {
        let mut index = 0usize;
        let mut output = DynamicStruct::default();

        let ignored_field_count = self.registration
            .type_meta::<SerializationData>()
            .map(|data| data.num_ignore_fields())
            .unwrap_or(0);
        let field_len = self.ty_info.num_fields().saturating_sub(ignored_field_count);

        if field_len == 0 {
            // handle unit structs and ignored fields
            return Ok(output);
        }

        while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
            self.ty_info.get_field_registration(index, self.registry)?,
            self.registry
        ))? {
            let name = self.ty_info.field_at(index).unwrap().name();

            output.add_field_boxed(name, value);
            index += 1;

            if index >= self.ty_info.num_fields() {
                break;
            }
        }

        Ok(output)
    }
}

pub(super) struct TupleStructVisitor<'a> {
    ty_info: &'static TupleStructTypeInfo,
    registry: &'a TypeRegistry,
    registration: &'a TypeRegistration,
}

impl<'a> TupleStructVisitor<'a> {
    pub fn new(
        ty_info: &'static TupleStructTypeInfo,
        registry: &'a TypeRegistry,
        registration: &'a TypeRegistration
    ) -> Self {
        Self { ty_info, registry, registration } 
    }
}

impl<'a, 'de> Visitor<'de> for TupleStructVisitor<'a> {
    type Value = DynamicTupleStruct;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected tuple struct value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut index = 0usize;
        let mut tuple_struct = DynamicTupleStruct::default();

        let ignored_len = self
            .registration
            .type_meta::<SerializationData>()
            .map(|data| data.num_ignore_fields())
            .unwrap_or(0);

        let field_len = self
            .ty_info
            .num_fields()
            .saturating_sub(ignored_len);

        if field_len == 0 {
            // Handle unit structs and ignored fields
            return Ok(tuple_struct);
        }

        let get_field_registration = |index: usize| -> Result<&'a TypeRegistration, V::Error> {
            let field = self.ty_info.field_at(index).ok_or_else(|| {
                Error::custom(format_args!(
                    "No field at index {} on tuple {}",
                    index,
                    self.ty_info.type_name(),
                ))
            })?;
            get_registration(field.type_id(), field.type_name(), self.registry)
        };

        while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
            get_field_registration(index)?,
            self.registry,
        ))? {
            tuple_struct.add_boxed(value);
            index += 1;
            if index >= self.ty_info.num_fields() {
                break;
            }
        }

        let ignored_len = self
            .registration
            .type_meta::<SerializationData>()
            .map(|data| data.num_ignore_fields())
            .unwrap_or(0);

        if tuple_struct.num_fields() != self.ty_info.num_fields() - ignored_len {
            return Err(Error::invalid_length(
                tuple_struct.num_fields(),
                &self.ty_info.num_fields().to_string().as_str(),
            ));
        }

        Ok(tuple_struct)
    }
}

pub(super) struct TupleVisitor<'a> {
    ty_info: &'static TupleTypeInfo,
    registry: &'a TypeRegistry,
}

impl<'a> TupleVisitor<'a> {
    pub fn new(
        ty_info: &'static TupleTypeInfo,
        registry: &'a TypeRegistry
    ) -> Self { 
        Self { ty_info, registry } 
    }
}

impl<'a, 'de> Visitor<'de> for TupleVisitor<'a> {
    type Value = DynamicTuple;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected tuple value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        visit_tuple(&mut seq, self.ty_info, self.registry)
    }
}

pub(super) struct EnumVisitor<'a> {
    ty_info: &'static EnumTypeInfo,
    registration: &'a TypeRegistration,
    registry: &'a TypeRegistry,
}

impl<'a> EnumVisitor<'a> {
    pub fn new(
        ty_info: &'static EnumTypeInfo,
        registration: &'a TypeRegistration,
        registry: &'a TypeRegistry
    ) -> Self { 
        Self { ty_info, registration, registry } 
    }
}

impl<'a, 'de> Visitor<'de> for EnumVisitor<'a> {
    type Value = DynamicEnum;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected enum value")
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::EnumAccess<'de>
    {
        let mut dynamic_enum = DynamicEnum::default();
        let (variant_info, variant) = data.variant_seed(
            VariantDeserializer::new(self.ty_info)
        )?;

        let value: DynamicVariant = match variant_info {
            VariantInfo::Unit(..) => variant.unit_variant()?.into(),
            VariantInfo::Struct(struct_info) => variant
                .struct_variant(
                    struct_info.field_names(),
                    StructVariantVisitor::new(
                        struct_info,
                        self.registration,
                        self.registry,
                    )
                )?
                .into(),
            VariantInfo::Tuple(tuple_info) if tuple_info.num_fields() == 1 => {
                let field = tuple_info.field_at(0).unwrap();
                let registration =
                    get_registration(field.type_id(), field.type_name(), self.registry)?;
                let value = variant.newtype_variant_seed(TypedReflectDeserializer::new(registration, self.registry))?;
                let mut dynamic_tuple = DynamicTuple::default();
                dynamic_tuple.add_field_boxed(value);
                dynamic_tuple.into()
            }
            VariantInfo::Tuple(tuple_info) => variant
                .tuple_variant(
                    tuple_info.num_fields(),
                    TupleVariantVisitor::new(
                        tuple_info,
                        self.registration,
                        self.registry,
                    ),
                )?
                .into(),
        };

        dynamic_enum.set_variant(variant_info.name(), value);
        Ok(dynamic_enum)
    }
}

pub(super) struct OptionVisitor<'a> {
    enum_info: &'static EnumTypeInfo,
    registry: &'a TypeRegistry,
}

impl<'a> OptionVisitor<'a> {
    pub fn new(
        enum_info: &'static EnumTypeInfo,
        registry: &'a TypeRegistry
    ) -> Self {
        Self { enum_info, registry } 
    }
}

impl<'a, 'de> Visitor<'de> for OptionVisitor<'a> {
    type Value = DynamicEnum;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected option value of type ")?;
        formatter.write_str(self.enum_info.type_name())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let variant_info = self.enum_info.variant("Some").unwrap();
        match variant_info {
            VariantInfo::Tuple(tuple_info) if tuple_info.num_fields() == 1 => {
                let field = tuple_info.field_at(0).unwrap();
                let registration =
                    get_registration(field.type_id(), field.type_name(), self.registry)?;
                let de = TypedReflectDeserializer::new(registration, self.registry);

                let mut value = DynamicTuple::default();
                value.add_field_boxed(de.deserialize(deserializer)?);

                let mut option = DynamicEnum::default();
                option.set_variant("Some", value);

                Ok(option)
            }
            info => Err(Error::custom(format_args!(
                "invalid variant, expected `Some` but got `{}`",
                info.name()
            ))),
        }
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let mut option = DynamicEnum::default();
        option.set_variant("None", ());
        Ok(option)
    }
}

/// Pretty much the same as [`TupleVisitor`].
/// Used to visit tuple variant in enum.
pub(super) struct TupleVariantVisitor<'a> {
    tuple_info: &'static TupleVariantInfo,
    registration: &'a TypeRegistration,
    registry: &'a TypeRegistry,
}

impl<'a> TupleVariantVisitor<'a> {
    pub fn new(
        tuple_info: &'static TupleVariantInfo,
        registration: &'a TypeRegistration,
        registry: &'a TypeRegistry,
    ) -> Self {
        Self {
            tuple_info,
            registration,
            registry,
        }
    }
}

impl<'a, 'de> Visitor<'de> for TupleVariantVisitor<'a> {
    type Value = DynamicTuple;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected tuple variant value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let ignored_len = self
            .registration
            .type_meta::<SerializationData>()
            .map(|data| data.num_ignore_fields())
            .unwrap_or(0);
        let field_len = self.tuple_info.num_fields().saturating_sub(ignored_len);

        if field_len == 0 {
            // Handle all fields being ignored
            return Ok(DynamicTuple::default());
        }

        visit_tuple(&mut seq, self.tuple_info, self.registry)
    }
}

/// Pretty much the same as [`StructVisitor`].
/// Used to visit struct variant in enum.
pub(super) struct StructVariantVisitor<'a> {
    struct_info: &'static StructVariantInfo,
    registration: &'a TypeRegistration,
    registry: &'a TypeRegistry,
}

impl<'a> StructVariantVisitor<'a> {
    pub fn new(
        struct_info: &'static StructVariantInfo,
        registration: &'a TypeRegistration,
        registry: &'a TypeRegistry
    ) -> Self {
        Self {
            struct_info,
            registration,
            registry,
        }
    }
}

impl<'a, 'de> Visitor<'de> for StructVariantVisitor<'a> {
    type Value = DynamicStruct;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected struct variant value")
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        visit_struct(&mut map, self.struct_info, self.registry)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut index = 0usize;
        let mut output = DynamicStruct::default();

        let ignored_len = self
            .registration
            .type_meta::<SerializationData>()
            .map(|data| data.num_ignore_fields())
            .unwrap_or(0);
        let field_len = self.struct_info.num_fields().saturating_sub(ignored_len);

        if field_len == 0 {
            // Handle all fields being ignored
            return Ok(output);
        }

        while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
            self.struct_info
                .get_field_registration(index, self.registry)?,
            self.registry
        ))? {
            let name = self.struct_info.field_at(index).unwrap().name();
            output.add_field_boxed(name, value);
            index += 1;
            if index >= self.struct_info.num_fields() {
                break;
            }
        }

        Ok(output)
    }
}

pub(super) struct ListVisitor<'a> {
    list_info: &'static ListTypeInfo,
    registry: &'a TypeRegistry,
}

impl<'a> ListVisitor<'a> {
    pub fn new(
        list_info: &'static ListTypeInfo,
        registry: &'a TypeRegistry
    ) -> Self {
        Self { list_info, registry }
    }
}

impl<'a, 'de> Visitor<'de> for ListVisitor<'a> {
    type Value = DynamicList;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected list value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut list = DynamicList::default();
        let registration = get_registration(
            self.list_info.item_type_id(),
            self.list_info.item_type_name(),
            self.registry,
        )?;
        while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
            registration,
            self.registry,
        ))? {
            list.push_boxed(value);
        }
        Ok(list)
    }
}

pub(super) struct MapVisitor<'a> {
    map_info: &'static MapTypeInfo,
    registry: &'a TypeRegistry,
}

impl<'a> MapVisitor<'a> {
    pub fn new(
        map_info: &'static MapTypeInfo,
        registry: &'a TypeRegistry
    ) -> Self {
        Self { map_info, registry }
    }
}

impl<'a, 'de> Visitor<'de> for MapVisitor<'a> {
    type Value = DynamicMap;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected map value")
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut dynamic_map = DynamicMap::default();
        let key_registration = get_registration(
            self.map_info.key_type_id(),
            self.map_info.key_type_name(),
            self.registry,
        )?;
        let value_registration = get_registration(
            self.map_info.value_type_id(),
            self.map_info.value_type_name(),
            self.registry,
        )?;
        while let Some(key) = map.next_key_seed(TypedReflectDeserializer::new(
            key_registration,
            self.registry,
        ))? {
            let value = map.next_value_seed(TypedReflectDeserializer::new(
                value_registration,
                self.registry,
            ))?;
            dynamic_map.insert_boxed(key, value);
        }

        Ok(dynamic_map)
    }
}

pub(super) struct ArrayVisitor<'a> {
    array_info: &'static ArrayTypeInfo,
    registry: &'a TypeRegistry,
}

impl<'a> ArrayVisitor<'a> {
    pub fn new(
        array_info: &'static ArrayTypeInfo,
        registry: &'a TypeRegistry
    ) -> Self {
        Self { array_info, registry }
    }
}

impl<'a, 'de> Visitor<'de> for ArrayVisitor<'a> {
    type Value = DynamicArray;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("reflected array value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or_default());
        let registration = get_registration(
            self.array_info.item_type_id(),
            self.array_info.item_type_name(),
            self.registry,
        )?;
        while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
            registration,
            self.registry,
        ))? {
            vec.push(value);
        }

        if vec.len() != self.array_info.capacity() {
            return Err(Error::invalid_length(
                vec.len(),
                &self.array_info.capacity().to_string().as_str(),
            ));
        }

        Ok(DynamicArray::new(vec.into_boxed_slice()))
    }
}

pub(super) fn visit_struct<'de, T, V>(
    map: &mut V,
    info: &'static T,
    registry: &TypeRegistry,
) -> Result<DynamicStruct, V::Error>
where
    T: StructLikeTypeInfo,
    V: MapAccess<'de>,
{
    let mut dynamic_struct = DynamicStruct::default();

    while let Some(Ident(key)) = map.next_key::<Ident>()? {
        let field = info.get_field(&key).ok_or_else(|| {
            let fields = info.iter().map(|field| field.name());
            Error::custom(format_args!(
                "unknown field `{}`, expected one of {:?}",
                key,
                ExpectedValues(fields.collect())
            ))
        })?;

        let registration = field_ty_registration::get_registration(field.type_id(), field.type_name(), registry)?;
        let value = map.next_value_seed(TypedReflectDeserializer::new(registration, registry))?;
        dynamic_struct.add_field_boxed(&key, value);
    }

    Ok(dynamic_struct)
}


fn visit_tuple<'de, T, V>(
    seq: &mut V,
    info: &T,
    registry: &TypeRegistry,
) -> Result<DynamicTuple, V::Error>
where
    T: TupleLikeTypeInfo,
    V: SeqAccess<'de>,
{
    let mut tuple = DynamicTuple::default();
    let mut index = 0usize;

    let get_field_registration = |index: usize| -> Result<&TypeRegistration, V::Error> {
        let field = info.get_field(index).ok_or_else(|| {
            Error::invalid_length(index, &info.num_fields().to_string().as_str())
        })?;
        get_registration(field.type_id(), field.type_name(), registry)
    };

    while let Some(value) = seq.next_element_seed(TypedReflectDeserializer::new(
        get_field_registration(index)?,
        registry
    ))? {
        tuple.add_field_boxed(value);
        index += 1;
        if index >= info.num_fields() {
            break;
        }
    }

    let len = info.num_fields();

    if tuple.num_fields() != len {
        return Err(Error::invalid_length(
            tuple.num_fields(),
            &len.to_string().as_str(),
        ));
    }

    Ok(tuple)
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Ident(String);

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdentVisitor;

        impl<'de> Visitor<'de> for IdentVisitor {
            type Value = Ident;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("identifier")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Ident(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Ident(value))
            }
        }

        deserializer.deserialize_identifier(IdentVisitor)
    }
}

/// A debug struct used for error messages that displays a list of expected values.
///
/// # Example
///
/// ```ignore
/// let expected = vec!["foo", "bar", "baz"];
/// assert_eq!("`foo`, `bar`, `baz`", format!("{}", ExpectedValues(expected)));
/// ```
pub(super) struct ExpectedValues<T: Display>(pub Vec<T>);

impl<T: Display> std::fmt::Debug for ExpectedValues<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.0.len();

        for (index, item) in self.0.iter().enumerate() {
            write!(f, "`{item}`")?;
            if index < len - 1 {
                write!(f, ", ")?;
            }
        }
        Ok(())
    }
}