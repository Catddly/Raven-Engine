use wyhash::WyHash as ReflectHasher;

extern crate log as glog; // to avoid name collision with my log module

mod type_info_cell;

mod reflect;
mod from_reflect;

mod std_traits;
mod impls;
mod type_info;
mod type_registry;
mod serde;

mod special_traits {
    pub(crate) mod debug;
    pub(crate) mod hash;
    pub(crate) mod partial_eq;
}

pub use reflect::{Reflect, ReflectRef, ReflectRefMut, ReflectOwned};
pub use from_reflect::*;

pub use type_info_cell::*;
pub use type_info::*;

pub use crate::serde::*;

#[cfg(test)]
mod tests {
    use std::collections::{VecDeque};

    use crate::{self as raven_reflect, type_info::Struct};
    use raven_reflect::*;
    use raven_reflect_derive::{Reflect};

    #[test]
    fn test_reflect_struct() {
        #[derive(Reflect)]
        struct Test {
            a: u32,
            b: char,
        }

        dbg!(Test::type_info());

        let test = Test {
            a: 123,
            b: 'a',
        };

        let field_a = test.field("a").unwrap().downcast_ref::<u32>().unwrap();
        let field_b = test.field("b").unwrap().downcast_ref::<char>().unwrap();

        assert_eq!(*field_a, 123); 
        assert_eq!(*field_b, 'a');
    }

    #[test]
    fn test_reflect_enum() {
        #[derive(Reflect)]
        enum Test {
            One,
            Two,
            Data(usize, u32),
            DataCompound {
                hello: usize,
                no: String,
                deque: VecDeque<u32>,
            }
        }

        dbg!(Test::type_info());

        let test_unit = Test::One;
        let test_tuple = Test::Data(368, 4);
        let mut test_struct = Test::DataCompound { hello: 7, no: String::from("Hello!"), deque: VecDeque::from([6, 3, 8, 7]) };

        assert!(test_unit.is_form(VariantForm::Unit));
        assert!(test_tuple.is_form(VariantForm::Tuple));
        assert!(test_struct.is_form(VariantForm::Struct));

        assert_eq!(0, test_unit.variant_index());
        assert_eq!(2, test_tuple.variant_index());
        assert_eq!(3, test_struct.variant_index());

        println!("test_unit::variant_name(): {}", test_unit.variant_name());
        println!("test_tuple::variant_name(): {}", test_tuple.variant_name());
        println!("test_struct::variant_name(): {}", test_struct.variant_name());

        println!("test_unit::variant_path(): {}", test_unit.variant_path());
        println!("test_tuple::variant_path(): {}", test_tuple.variant_path());
        println!("test_struct::variant_path(): {}", test_struct.variant_path());

        assert_eq!(368, *test_tuple.field_at(0).unwrap().downcast_ref::<usize>().unwrap());
        assert_eq!(4, *test_tuple.field_at(1).unwrap().downcast_ref::<u32>().unwrap());

        println!("origin no string: {}", if let Test::DataCompound { no, .. } = &test_struct { 
            no 
        } else { 
            panic!("test_struct is not Test::DataCompound") 
        });

        let no_field = test_struct.field_mut("no").unwrap().downcast_mut::<String>().unwrap();
        no_field.push_str("Bye!");

        println!("modified no string: {}", if let Test::DataCompound { no, .. } = &test_struct { 
            no 
        } else { 
            panic!("test_struct is not Test::DataCompound") 
        });
    }

    #[test]
    fn test_reflect_tuple_struct() {
        #[derive(Reflect)]
        struct TestTupleStruct(usize, String, Vec<i32>);

        dbg!(TestTupleStruct::type_info());

        let test_struct = TestTupleStruct(645, String::from("Hello!"), vec![-85, 69, 15]);

        let field_0 = test_struct.field_at(0).unwrap().downcast_ref::<usize>().unwrap();
        let field_1 = test_struct.field_at(1).unwrap().downcast_ref::<String>().unwrap();
        let field_2 = test_struct.field_at(2).unwrap().downcast_ref::<Vec<i32>>().unwrap();

        assert_eq!(*field_0, 645);
        assert_eq!(*field_1, String::from("Hello!"));
        assert_eq!(*field_2, vec![-85, 69, 15]);
    }
}