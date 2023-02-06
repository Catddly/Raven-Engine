use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::num::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize, NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize};
use std::ops::{Range, RangeInclusive, RangeFrom, RangeTo, RangeToInclusive, RangeFull};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{
    Reflect, FromReflect, Typed, TypeInfo, GenericTypeInfoOnceCell, ListTypeInfo,
    type_registry::{GetTypeRegistration, TypeRegistration, ReflectFromPtr, FromType},
    Array, ArrayIter, List,
    ReflectRef, ReflectRefMut, ReflectOwned,
};
use crate::{self as raven_reflect, UnnamedField, VariantInfo, UnitVariantInfo, TupleVariantInfo, EnumTypeInfo, Enum, DynamicEnum, VariantForm, VariantFieldIter};
use raven_reflect_derive::{impl_reflect_primitive, impl_from_reflect_primitive};

use crate::std_traits::ReflectDefault;
use crate::serde::{ReflectSerialize, ReflectDeserialize};

impl_reflect_primitive!(u8(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(u16(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(u32(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(u64(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(u128(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(usize(Debug, Hash, PartialEq, Serialize, Deserialize, Default));

impl_reflect_primitive!(i8(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(i16(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(i32(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(i64(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(i128(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(isize(Debug, Hash, PartialEq, Serialize, Deserialize, Default));

impl_reflect_primitive!(f32(Debug, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(f64(Debug, PartialEq, Serialize, Deserialize, Default));

impl_reflect_primitive!(bool(Debug, Hash, PartialEq, Serialize, Deserialize, Default));
impl_reflect_primitive!(char(Debug, Hash, PartialEq, Serialize, Deserialize, Default));

impl_reflect_primitive!(String(
    Debug,
    Hash,
    PartialEq,
    Serialize,
    Deserialize,
    Default
));

impl_reflect_primitive!(PathBuf(
    Debug,
    Hash,
    PartialEq,
    Serialize,
    Deserialize,
    Default
));

impl_reflect_primitive!(Result<T: Clone + Reflect + 'static, E: Clone + Reflect + 'static>());
impl_reflect_primitive!(HashSet<T: Hash + Eq + Clone + Send + Sync + 'static>());

impl_reflect_primitive!(Range<T: Clone + Send + Sync + 'static>());
impl_reflect_primitive!(RangeInclusive<T: Clone + Send + Sync + 'static>());
impl_reflect_primitive!(RangeFrom<T: Clone + Send + Sync + 'static>());
impl_reflect_primitive!(RangeTo<T: Clone + Send + Sync + 'static>());
impl_reflect_primitive!(RangeToInclusive<T: Clone + Send + Sync + 'static>());
impl_reflect_primitive!(RangeFull());

impl_reflect_primitive!(Duration(
    Debug,
    Hash,
    PartialEq,
    Serialize,
    Deserialize,
    Default
));
impl_reflect_primitive!(Instant(Debug, Hash, PartialEq));

impl_reflect_primitive!(NonZeroIsize(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroI128(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroI64(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroI32(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroI16(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroI8(Debug, Hash, PartialEq, Serialize, Deserialize));

impl_reflect_primitive!(NonZeroUsize(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroU128(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroU64(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroU32(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroU16(Debug, Hash, PartialEq, Serialize, Deserialize));
impl_reflect_primitive!(NonZeroU8(Debug, Hash, PartialEq, Serialize, Deserialize));

impl_from_reflect_primitive!(u8);
impl_from_reflect_primitive!(u16);
impl_from_reflect_primitive!(u32);
impl_from_reflect_primitive!(u64);
impl_from_reflect_primitive!(u128);
impl_from_reflect_primitive!(usize);
impl_from_reflect_primitive!(i8);
impl_from_reflect_primitive!(i16);
impl_from_reflect_primitive!(i32);
impl_from_reflect_primitive!(i64);
impl_from_reflect_primitive!(i128);
impl_from_reflect_primitive!(isize);
impl_from_reflect_primitive!(f32);
impl_from_reflect_primitive!(f64);
impl_from_reflect_primitive!(bool);
impl_from_reflect_primitive!(char);
impl_from_reflect_primitive!(String);
impl_from_reflect_primitive!(PathBuf);
impl_from_reflect_primitive!(Result<T: Clone + Reflect + 'static, E: Clone + Reflect + 'static>);
impl_from_reflect_primitive!(HashSet<T: Hash + Eq + Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(Range<T: Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(RangeInclusive<T: Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(RangeFrom<T: Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(RangeTo<T: Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(RangeToInclusive<T: Clone + Send + Sync + 'static>);
impl_from_reflect_primitive!(RangeFull);
impl_from_reflect_primitive!(Duration);
impl_from_reflect_primitive!(Instant);
impl_from_reflect_primitive!(NonZeroIsize);
impl_from_reflect_primitive!(NonZeroI128);
impl_from_reflect_primitive!(NonZeroI64);
impl_from_reflect_primitive!(NonZeroI32);
impl_from_reflect_primitive!(NonZeroI16);
impl_from_reflect_primitive!(NonZeroI8);
impl_from_reflect_primitive!(NonZeroUsize);
impl_from_reflect_primitive!(NonZeroU128);
impl_from_reflect_primitive!(NonZeroU64);
impl_from_reflect_primitive!(NonZeroU32);
impl_from_reflect_primitive!(NonZeroU16);
impl_from_reflect_primitive!(NonZeroU8);

macro_rules! impl_reflect_veclike {
    ($ty:ty, $push:expr, $pop:expr, $insert:expr, $remove:expr, $sub:ty) => {
        impl<T: FromReflect> Typed for $ty {
            fn type_info() -> &'static TypeInfo {
                static TYPE_INFO_CELL: GenericTypeInfoOnceCell = GenericTypeInfoOnceCell::new();
                TYPE_INFO_CELL.get_or_insert::<Self, _>(|| TypeInfo::List(ListTypeInfo::new::<Self, T>()))
            }
        }

        impl<T: FromReflect> GetTypeRegistration for $ty {
            fn get_type_registration() -> TypeRegistration {
                let mut registration = TypeRegistration::type_of::<$ty>();
                registration.insert::<ReflectFromPtr>(FromType::<$ty>::from_type());
                registration
            }
        }

        impl<T: FromReflect> Array for $ty {
            #[inline]
            fn get(&self, index: usize) -> Option<&dyn Reflect> {
                <$sub>::get(self, index).map(|value| value as &dyn Reflect)
            }

            #[inline]
            fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
                <$sub>::get_mut(self, index).map(|value| value as &mut dyn Reflect)
            }

            #[inline]
            fn len(&self) -> usize {
                <$sub>::len(self)
            }

            #[inline]
            fn iter(&self) -> ArrayIter {
                ArrayIter::new(self)
            }

            #[inline]
            fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
                self.into_iter()
                    .map(|value| Box::new(value) as Box<dyn Reflect>)
                    .collect()
            }
        }

        impl<T: FromReflect> List for $ty {
            fn insert(&mut self, index: usize, element: Box<dyn Reflect>) {
                let value = element.take::<T>().unwrap_or_else(|value| {
                    T::from_reflect(&*value).unwrap_or_else(|| {
                        panic!(
                            "Attempted to insert invalid value of type {}.",
                            value.type_name()
                        )
                    })
                });
                $insert(self, index, value);
            }
        
            fn remove(&mut self, index: usize) -> Box<dyn Reflect> {
                Box::new($remove(self, index))
            }

            fn push(&mut self, value: Box<dyn Reflect>) {
                let value = value.take::<T>().unwrap_or_else(|value| {
                    T::from_reflect(&*value).unwrap_or_else(|| {
                        panic!(
                            "Attempted to push invalid value of type {}.",
                            value.type_name()
                        )
                    })
                });
                $push(self, value);
            }

            fn pop(&mut self) -> Option<Box<dyn Reflect>> {
                $pop(self).map(|value| Box::new(value) as Box<dyn Reflect>)
            }
        }

        impl<T: FromReflect> Reflect for $ty {
            #[inline]
            fn type_name(&self) -> &'static str {
                ::core::any::type_name::<Self>()
            }

            #[inline]
            fn get_type_info(&self) -> &'static TypeInfo {
                <Self as Typed>::type_info()
            }
        
            #[inline]
            fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> {
                self
            }

            #[inline]
            fn as_reflect(&self) -> &dyn Reflect {
                self
            }

            #[inline]
            fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
                self
            }

            #[inline]
            fn clone_value(&self) -> Box<dyn Reflect> {
                Box::new(List::clone_dynamic(self))
            }

            fn assign(&mut self, value: &dyn Reflect) {
                crate::type_info::list_assign(self, value);
            }

            fn reflect_ref(&self) -> ReflectRef {
                ReflectRef::List(self)
            }

            fn reflect_ref_mut(&mut self) -> ReflectRefMut {
                ReflectRefMut::List(self)
            }

            fn reflect_owned(self: Box<Self>) -> ReflectOwned {
                ReflectOwned::List(self)
            }

            fn reflect_hash(&self) -> Option<u64> {
                crate::special_traits::hash::array_hash(self)
            }

            fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
                crate::special_traits::partial_eq::list_partial_eq(self, value)
            }
        }

        impl<T: FromReflect> FromReflect for $ty {
            fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
                if let ReflectRef::List(ref_list) = reflect.reflect_ref() {
                    let mut new_list = Self::with_capacity(ref_list.len());
                    for field in ref_list.iter() {
                        $push(&mut new_list, T::from_reflect(field)?);
                    }
                    Some(new_list)
                } else {
                    None
                }
            }
        }
    };
}

impl_reflect_veclike!(Vec<T>, Vec::push, Vec::pop, Vec::insert, Vec::remove, [T]);
impl_reflect_veclike!(
    VecDeque<T>,
    VecDeque::push_back, VecDeque::pop_back,
    VecDeque::insert, VecDeque::remove,
    VecDeque::<T>
);


impl<T: FromReflect> Typed for Option<T> {
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoOnceCell = GenericTypeInfoOnceCell::new();
        CELL.get_or_insert::<Self, _>(|| {
            let none_variant = VariantInfo::Unit(UnitVariantInfo::new("None"));
            let some_variant =
                VariantInfo::Tuple(TupleVariantInfo::new("Some", &[UnnamedField::new::<T>(0)]));
            TypeInfo::Enum(EnumTypeInfo::new::<Self>(
                "Option",
                &[none_variant, some_variant],
            ))
        })
    }
}

impl<T: FromReflect> GetTypeRegistration for Option<T> {
    fn get_type_registration() -> TypeRegistration {
        TypeRegistration::type_of::<Option<T>>()
    }
}

impl<T: FromReflect> Enum for Option<T> {
    fn field(&self, _name: &str) -> Option<&dyn Reflect> {
        None
    }

    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        match self {
            Some(value) if index == 0 => Some(value),
            _ => None,
        }
    }

    fn field_mut(&mut self, _name: &str) -> Option<&mut dyn Reflect> {
        None
    }

    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        match self {
            Some(value) if index == 0 => Some(value),
            _ => None,
        }
    }

    fn index_of(&self, _name: &str) -> Option<usize> {
        None
    }

    fn field_name_at(&self, _index: usize) -> Option<&str> {
        None
    }

    fn iter(&self) -> VariantFieldIter {
        VariantFieldIter::new(self)
    }

    #[inline]
    fn num_fields(&self) -> usize {
        match self {
            Some(..) => 1,
            None => 0,
        }
    }

    #[inline]
    fn variant_name(&self) -> &str {
        match self {
            Some(..) => "Some",
            None => "None",
        }
    }

    fn variant_index(&self) -> usize {
        match self {
            None => 0,
            Some(..) => 1,
        }
    }

    #[inline]
    fn variant_form(&self) -> VariantForm {
        match self {
            Some(..) => VariantForm::Tuple,
            None => VariantForm::Unit,
        }
    }

    fn clone_dynamic(&self) -> DynamicEnum {
        DynamicEnum::from_ref::<Self>(self)
    }
}

impl<T: FromReflect> Reflect for Option<T> {
    #[inline]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    #[inline]
    fn get_type_info(&self) -> &'static TypeInfo {
        <Self as Typed>::type_info()
    }

    #[inline]
    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> {
        self
    }

    fn as_reflect(&self) -> &dyn Reflect {
        self
    }

    fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
        self
    }

    #[inline]
    fn assign(&mut self, value: &dyn Reflect) {
        if let ReflectRef::Enum(value) = value.reflect_ref() {
            if self.variant_name() == value.variant_name() {
                // Same variant -> just update fields
                for (index, field) in value.iter().enumerate() {
                    if let Some(v) = self.field_at_mut(index) {
                        v.assign(field.value());
                    }
                }
            } else {
                // New variant -> perform a switch
                match value.variant_name() {
                    "Some" => {
                        let field = T::take_from_reflect(
                            value
                                .field_at(0)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "Field in `Some` variant of {} should exist",
                                        std::any::type_name::<Option<T>>()
                                    )
                                })
                                .clone_value(),
                        )
                        .unwrap_or_else(|_| {
                            panic!(
                                "Field in `Some` variant of {} should be of type {}",
                                std::any::type_name::<Option<T>>(),
                                std::any::type_name::<T>()
                            )
                        });
                        *self = Some(field);
                    }
                    "None" => {
                        *self = None;
                    }
                    _ => panic!("Enum is not a {}.", std::any::type_name::<Self>()),
                }
            }
        }
    }

    fn reflect_ref(&self) -> ReflectRef {
        ReflectRef::Enum(self)
    }

    fn reflect_ref_mut(&mut self) -> ReflectRefMut {
        ReflectRefMut::Enum(self)
    }

    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::Enum(self)
    }

    #[inline]
    fn clone_value(&self) -> Box<dyn Reflect> {
        Box::new(Enum::clone_dynamic(self))
    }

    fn reflect_hash(&self) -> Option<u64> {
        crate::special_traits::hash::enum_hash(self)
    }

    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::special_traits::partial_eq::enum_partial_eq(self, value)
    }
}

impl<T: FromReflect> FromReflect for Option<T> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Enum(dyn_enum) = reflect.reflect_ref() {
            match dyn_enum.variant_name() {
                "Some" => {
                    let field = T::take_from_reflect(
                        dyn_enum
                            .field_at(0)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Field in `Some` variant of {} should exist",
                                    std::any::type_name::<Option<T>>()
                                )
                            })
                            .clone_value(),
                    )
                    .unwrap_or_else(|_| {
                        panic!(
                            "Field in `Some` variant of {} should be of type {}",
                            std::any::type_name::<Option<T>>(),
                            std::any::type_name::<T>()
                        )
                    });
                    Some(Some(field))
                }
                "None" => Some(None),
                name => panic!(
                    "variant with name `{}` does not exist on enum `{}`",
                    name,
                    std::any::type_name::<Self>()
                ),
            }
        } else {
            None
        }
    }
}