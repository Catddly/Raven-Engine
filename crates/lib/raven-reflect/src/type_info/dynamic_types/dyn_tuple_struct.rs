use std::fmt::Formatter;

use crate::{Reflect, Typed, TypeInfo, NonGenericTypeInfoOnceCell, DynamicTypeInfo, TupleStruct, special_traits::{debug, partial_eq}};

/// A tuple struct which allow fields to be modified at runtime. 
#[derive(Default)]
pub struct DynamicTupleStruct {
    name: String,
    fields: Vec<Box<dyn Reflect>>,
}

impl DynamicTupleStruct {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn add_boxed(&mut self, value: Box<dyn Reflect>) {
        self.fields.push(value);
    }

    pub fn add<T: Reflect>(&mut self, value: T) {
        self.add_boxed(Box::new(value));
    }
}

// impl Typed
impl Typed for DynamicTupleStruct {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(
            DynamicTypeInfo::new::<Self>()
        ))
    }
}

// impl Debug
impl std::fmt::Debug for DynamicTupleStruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

// impl TupleStruct
impl TupleStruct for DynamicTupleStruct {
    #[inline]
    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index).map(|field| &**field)
    }

    #[inline]
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index).map(|field| &mut **field)
    }

    #[inline]
    fn num_fields(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn iter(&self) -> crate::TupleStructFieldIter {
        crate::TupleStructFieldIter::new(self)
    }

    #[inline]
    fn clone_dynamic(&self) -> DynamicTupleStruct {
        DynamicTupleStruct {
            name: self.name.clone(),
            fields: self.fields
                .iter()
                .map(|value| value.clone_value())
                .collect(),
        }
    }
}

// impl Reflect
impl Reflect for DynamicTupleStruct {
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
        Box::new(self.clone_dynamic())
    }

    #[inline]
    fn assign(&mut self, reflected: &dyn Reflect) {
        if let crate::ReflectRef::TupleStruct(tuple_struct) = reflected.reflect_ref() {
            for (i, value) in tuple_struct.iter().enumerate() {
                if let Some(v) = self.field_at_mut(i) {
                    v.assign(value);
                }
            }
        } else {
            panic!("Attempted to apply non-TupleStruct type to TupleStruct type.");
        }
    }
    
    fn reflect_ref<'a>(&'a self) -> crate::ReflectRef<'a> {
        crate::ReflectRef::TupleStruct(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> crate::ReflectRefMut<'a> {
        crate::ReflectRefMut::TupleStruct(self)
    }
    
    fn reflect_owned<'a>(self: Box<Self>) -> crate::ReflectOwned {
        crate::ReflectOwned::TupleStruct(self)
    }
    
    /// #partial_eq_impl
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::tuple_struct_partial_eq(self, value)
    }

    /// #debug_impl
    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicTupleStruct(")?;
        debug::tuple_struct_debug(self, f)?;
        write!(f, ")")
    }
}