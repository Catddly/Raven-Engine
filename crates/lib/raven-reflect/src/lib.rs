use std::any::Any;

use downcast_rs::{DowncastSync, impl_downcast};

extern crate log as glog; // to avoid name collision with my log module

mod type_info_cell;

mod impls;
mod type_info;
mod type_registry;

pub use type_info_cell::*;
pub use type_info::{Typed, TypeInfo};

pub trait Reflect: DowncastSync {
    fn type_name(&self) -> &'static str;

    //fn type_info(&self) -> &'static TypeInfo;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect>;
    fn as_reflect(&self) -> &dyn Reflect;
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect;
}

impl_downcast!(sync Reflect);

#[cfg(test)]
mod tests {
    use crate::{self as raven_reflect, type_info::Struct};
    use raven_reflect::*;
    use raven_reflect_derive::Reflect;

    #[test]
    fn test_reflect_derive() {
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

        //test_reflect_crate_path();
    }
}