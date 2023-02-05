mod serializer;
mod reflect_ser;

mod variant_deserializer;
mod deserializer;
mod reflect_de;

mod visitors;
mod field_ty_registration;

pub use serializer::ReflectSerializer;
pub use reflect_ser::{SerializationData, ReflectSerialize};

pub use deserializer::{UntypedReflectDeserializer, TypedReflectDeserializer};
pub use reflect_de::ReflectDeserialize;

#[cfg(test)]
mod tests {
    use std::{ops::Range};

    use crate::{self as raven_reflect,
        type_registry::TypeRegistry,
        serde::{ReflectSerializer, deserializer::UntypedReflectDeserializer},
        Reflect, Typed, std_traits::ReflectDefault
    };
    use raven_reflect_derive::Reflect;
    use ron::ser::PrettyConfig;
    use serde::de::DeserializeSeed;

    #[test]
    fn test_serialize_struct() {
        #[derive(Reflect, Default, Debug)]
        #[reflect(Default)]
        struct TestStruct {
            a: u32,
            #[reflect(transparent)]
            b: char,
            c: i32,
            #[reflect(no_serialization)]
            d: bool,
            #[reflect(no_serialization)]
            range: Range<u32>,
            str: String,
        }

        let mut registry = TypeRegistry::default();
        registry.register::<TestStruct>();
        registry.register::<String>();

        // test_struct can be constructed from pointer.
        let test_struct = TestStruct {
            a: 3,
            b: 'g',
            c: -5,
            d: false,
            range: 3..8,
            str: String::from("TestStruct!"),
        };

        let serializer = ReflectSerializer::new(&test_struct, &registry);
        let sered_str = ron::ser::to_string_pretty(&serializer, PrettyConfig::default())
            .expect("Failed to serialize reflected struct `Test`!");
        
        println!("Reflected:  {:#?}", test_struct.as_reflect());
        println!("Serialized: {sered_str}");

        let mut deserializer = ron::de::Deserializer::from_str(&sered_str)
            .expect("Failed to parse ron!");
        let reflect_deserializer = UntypedReflectDeserializer::new(&registry);
        let reflected = reflect_deserializer.deserialize(&mut deserializer)
            .expect("Failed to deserialize reflected struct!");

        let type_meta = registry.type_meta::<ReflectDefault>(TestStruct::type_info().type_id()).unwrap();
        let test_default = type_meta.default();
        println!("Default Test from ReflectDefault: {test_default:#?}");

        let mut default_test = TestStruct::default();
        println!("Default Test: {default_test:#?}");

        default_test.assign(&*reflected);
        println!("Assigned Test: {default_test:#?}");
    }

    #[test]
    fn test_serialize_enum() {
        #[derive(Reflect, Default, Debug)]
        #[reflect(Default)]
        enum TestEnum {
            #[default]
            Unit,
            Tuple1(usize),
            Tuple2(String, i32),
            Tuple3(i32),
            Struct1 {
                id: u32,
                name: String,
            }
        }

        let mut registry = TypeRegistry::default();
        registry.register::<TestEnum>();
        registry.register::<String>();

        // test_struct can be constructed from pointer.
        let test_enum = TestEnum::Tuple2(String::from("Hello!"), -2);

        let serializer = ReflectSerializer::new(&test_enum, &registry);
        let sered_str = ron::ser::to_string_pretty(&serializer, PrettyConfig::default())
            .expect("Failed to serialize reflected enum `TestEnum`!");
        
        println!("Reflected:  {:#?}", test_enum.as_reflect());
        println!("Serialized: {sered_str}");

        let mut deserializer = ron::de::Deserializer::from_str(&sered_str)
            .expect("Failed to parse ron!");
        let reflect_deserializer = UntypedReflectDeserializer::new(&registry);
        let reflected = reflect_deserializer.deserialize(&mut deserializer)
            .expect("Failed to deserialize reflected struct!");

        let type_meta = registry.type_meta::<ReflectDefault>(TestEnum::type_info().type_id()).unwrap();
        let test_default = type_meta.default();
        println!("Default TestEnum from ReflectDefault: {test_default:#?}");

        let mut default_test = TestEnum::default();
        println!("Default TestEnum: {default_test:#?}");

        default_test.assign(&*reflected);
        println!("Assigned TestEnum: {default_test:#?}");
    }
}