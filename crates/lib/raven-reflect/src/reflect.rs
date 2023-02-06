use std::{fmt::Debug};

use downcast_rs::{DowncastSync, impl_downcast};

use crate::{type_info::{Struct, PrimitiveTypeInfo}, TypeInfo, special_traits, NonGenericTypeInfoOnceCell, Typed, Enum, Tuple, TupleStruct, List, Map, Array};

/// Wrapper enum to get a immutable reference of reflected data conveniently.
/// This helper class classify reflected data for user. 
pub enum ReflectRef<'a> {
    Struct(&'a dyn Struct),
    TupleStruct(&'a dyn TupleStruct),
    Tuple(&'a dyn Tuple),
    Enum(&'a dyn Enum),
    Array(&'a dyn Array),
    List(&'a dyn List),
    Map(&'a dyn Map),
    Primitive(&'a dyn Reflect),
}

/// Wrapper enum to get a mutable reference of reflected data conveniently.
/// This helper class classify reflected data for user. 
pub enum ReflectRefMut<'a> {
    Struct(&'a mut dyn Struct),
    TupleStruct(&'a mut dyn TupleStruct),
    Tuple(&'a mut dyn Tuple),
    Enum(&'a mut dyn Enum),
    Array(&'a mut dyn Array),
    List(&'a mut dyn List),
    Map(&'a mut dyn Map),
    Primitive(&'a mut dyn Reflect),
}

/// Wrapper enum to get a owned reflected data conveniently.
/// This helper class classify reflected data for user. 
pub enum ReflectOwned {
    Struct(Box<dyn Struct>),
    TupleStruct(Box<dyn TupleStruct>),
    Tuple(Box<dyn Tuple>),
    Enum(Box<dyn Enum>),
    Array(Box<dyn Array>),
    List(Box<dyn List>),
    Map(Box<dyn Map>),
    Primitive(Box<dyn Reflect>),
}

pub trait Reflect: DowncastSync {
    fn type_name(&self) -> &'static str;

    fn get_type_info(&self) -> &'static TypeInfo;

    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect>;
    fn as_reflect(&self) -> &dyn Reflect;
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect;

    fn reflect_ref<'a>(&'a self) -> ReflectRef<'a>;
    fn reflect_ref_mut<'a>(&'a mut self) -> ReflectRefMut<'a>;
    fn reflect_owned<'a>(self: Box<Self>) -> ReflectOwned;

    fn assign(&mut self, reflected: &dyn Reflect);

    /// Clones the value as a `Reflect` trait object.
    ///
    /// When deriving `Reflect` for a struct, tuple struct or enum, the value is
    /// cloned via [`Struct::clone_dynamic`], [`TupleStruct::clone_dynamic`],
    /// or [`Enum::clone_dynamic`], respectively.
    /// Implementors of other `Reflect` subtraits (e.g. [`List`], [`Map`]) should
    /// use those subtraits' respective `clone_dynamic` methods.
    fn clone_value(&self) -> Box<dyn Reflect>;

    /// Debug formatter for the value.
    ///
    /// Any value that is not an implementor of other `Reflect` subtraits
    /// (e.g. [`List`], [`Map`]), will default to the format: `"Reflect(type_name)"`,
    /// where `type_name` is the [type name] of the underlying type.
    ///
    /// [type name]: Self::type_name
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.reflect_ref() {
            ReflectRef::Struct(dyn_struct) => special_traits::debug::struct_debug(dyn_struct, f),
            _ => write!(f, "Reflect({})", self.type_name()),
        }
    }
    
    /// Returns a hash of the value (which includes the type).
    ///
    /// If the underlying type does not support hashing, returns `None`.
    fn reflect_hash(&self) -> Option<u64> {
        None
    }

    /// Returns a "partial equality" comparison result.
    ///
    /// If the underlying type does not support equality testing, returns `None`.
    fn reflect_partial_eq(&self, _value: &dyn Reflect) -> Option<bool> {
        None
    }
}

impl_downcast!(sync Reflect);

impl Debug for dyn Reflect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

impl Typed for dyn Reflect {
    fn type_info() -> &'static TypeInfo {
        static CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        CELL.get_or_set(|| TypeInfo::Primitive(PrimitiveTypeInfo::new::<Self>()))
    }
}

impl dyn Reflect {
    /// Try to downcast dyn Reflect to concrete type T.
    /// If failed, return origin Box<dyn Reflect>
    pub fn take<T: Reflect>(self: Box<dyn Reflect>) -> Result<T, Box<dyn Reflect>> {
        self.downcast::<T>().map(|v| *v)
    }
}