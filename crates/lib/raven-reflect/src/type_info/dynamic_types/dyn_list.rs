use std::fmt::Formatter;

use syn::token::Dyn;

use crate::{Reflect, Array, List, TypeInfo, Typed, NonGenericTypeInfoOnceCell, DynamicTypeInfo, ReflectRef, special_traits::{debug, partial_eq}};

/// A runtime extensible array of reflected values.
#[derive(Default)]
pub struct DynamicList {
    name: String,
    values: Vec<Box<dyn Reflect>>,
}

impl DynamicList {
    pub(crate) fn new(name: String, values: Vec<Box<dyn Reflect>>) -> DynamicList {
        Self {
            name,
            values,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn push<T: Reflect>(&mut self, value: T) {
        self.values.push(Box::new(value));
    }

    pub fn push_boxed(&mut self, value: Box<dyn Reflect>) {
        self.values.push(value);
    }
}

// impl Typed
impl Typed for DynamicList {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(
            DynamicTypeInfo::new::<Self>()
        ))
    }
}

// impl Debug
impl std::fmt::Debug for DynamicList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

// impl Array
impl Array for DynamicList {
    #[inline]
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        self.values.get(index).map(|v| &**v)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.values.get_mut(index).map(|v| &mut **v)
    }

    #[inline]
    fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn iter(&self) -> crate::ArrayIter {
        crate::ArrayIter::new(self)
    }

    fn clone_dynamic(&self) -> crate::DynamicArray {
        let mut dyn_aray = crate::DynamicArray::new(self.values
            .iter()
            .map(|value| value.clone_value())
            .collect()
        );
        dyn_aray.set_name(self.name.clone());
        dyn_aray
    }
}

// impl List
impl List for DynamicList {
    #[inline]
    fn push(&mut self, value: Box<dyn Reflect>) {
        self.values.push(value)
    }

    #[inline]
    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        self.values.pop()
    }

    fn clone_dynamic(&self) -> DynamicList {
        DynamicList {
            name: self.name.clone(),
            values: self
                .values
                .iter()
                .map(|value| value.clone_value())
                .collect(),
        }
    }
}

// impl Reflect
impl Reflect for DynamicList {
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

    fn assign(&mut self, reflected: &dyn Reflect) {
        if let ReflectRef::List(list_value) = reflected.reflect_ref() {
            for (i, value) in list_value.iter().enumerate() {
                if i < self.len() {
                    if let Some(v) = self.get_mut(i) {
                        v.assign(value);
                    }
                } else {
                    List::push(self, value.clone_value());
                }
            }
        } else {
            panic!("Attempted to apply a non-list type to a list type.");
        }
    }

    fn reflect_ref<'a>(&'a self) -> crate::ReflectRef<'a> {
        crate::ReflectRef::List(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> crate::ReflectRefMut<'a> {
        crate::ReflectRefMut::List(self)
    }

    fn reflect_owned<'a>(self: Box<Self>) -> crate::ReflectOwned {
        crate::ReflectOwned::List(self)
    }
    
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::list_partial_eq(self, value)
    }

    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicList(")?;
        debug::list_debug(self, f)?;
        write!(f, ")")
    }
}