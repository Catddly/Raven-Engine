use std::collections::HashSet;
use std::hash::Hash;
use std::num::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize, NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize};
use std::ops::{Range, RangeInclusive, RangeFrom, RangeTo, RangeToInclusive, RangeFull};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{self as raven_reflect, Reflect};
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