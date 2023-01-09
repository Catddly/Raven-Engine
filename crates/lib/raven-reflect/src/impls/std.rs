use crate as raven_reflect;
use raven_reflect_derive::impl_reflect_primitive;

impl_reflect_primitive!(u8);
impl_reflect_primitive!(u16);
impl_reflect_primitive!(u32);
impl_reflect_primitive!(u64);
impl_reflect_primitive!(u128);
impl_reflect_primitive!(usize);

impl_reflect_primitive!(i8);
impl_reflect_primitive!(i16);
impl_reflect_primitive!(i32);
impl_reflect_primitive!(i64);
impl_reflect_primitive!(i128);
impl_reflect_primitive!(isize);

impl_reflect_primitive!(f32);
impl_reflect_primitive!(f64);

impl_reflect_primitive!(bool);
impl_reflect_primitive!(char);