use std::{borrow::Cow, collections::{HashMap, hash_map::Entry}, fmt::{Formatter, Debug}};

use crate::{
    type_info::{
        Struct, StructFieldIter, DynamicTypeInfo
    }, Reflect, Typed, TypeInfo, NonGenericTypeInfoOnceCell,
    ReflectRef, ReflectRefMut, ReflectOwned,
    special_traits::{debug, partial_eq},
};

/// A struct which allow fields to be modified at runtime.
/// 
/// With comparison to compile-time Struct, there are several differences:
/// 
/// * Not implemented reflect_hash for Reflect.
/// * Not implemented GetTypeRegistration.
/// 
/// The reason is we treat arbitrary [`DynamicStruct`] as one single type with one single type_id,
/// so register multiple DynamicStruct will be ambiguous.
/// That's why we don't call register::<DynamicStruct>() on type registry,
/// because this function will call trait [`GetTypeRegistration`] which has no implementation.
/// 
/// [`GetTypeRegistration`]: crate::type_registry::GetTypeRegistration
#[derive(Default)]
pub struct DynamicStruct {
    name: String,
    fields: Vec<Box<dyn Reflect>>,
    field_names: Vec<Cow<'static, str>>,
    field_indices: HashMap<Cow<'static, str>, usize>,   
}

impl DynamicStruct {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.field_indices.get(name).copied()
    } 

    pub fn add_field_boxed(&mut self, name: &str, field: Box<dyn Reflect>) {
        let name = Cow::Owned(name.to_string());

        match self.field_indices.entry(name) {
            Entry::Occupied(entry) => {
                self.fields[*entry.get()] = field;
            }
            Entry::Vacant(entry) => {
                self.fields.push(field);
                self.field_names.push(entry.key().clone());
                entry.insert(self.fields.len() - 1);
            }
        }
    }
    
    pub fn add_field(&mut self, name: &str, field: impl Reflect) {
        if let Some(index) = self.field_indices.get(name) {
            self.fields[*index] = Box::new(field);
        } else {
            self.add_field_boxed(name, Box::new(field));
        }
    }
}

// implement Typed
impl Typed for DynamicStruct {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(
            DynamicTypeInfo::new::<Self>()
        ))
    }
}

// implement Struct
impl Struct for DynamicStruct {
    #[inline]
    fn field(&self, name: &str) -> Option<&dyn Reflect> {
        self.field_indices.get(name)
            .map(|index| &*self.fields[*index] )
    }

    #[inline]
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect> {
        self.field_indices.get(name)
            .map(|index| &mut *self.fields[*index] )
    }

    #[inline]
    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index)
            .map(|field| &**field)
    }

    #[inline]
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index)
            .map(|field| &mut **field )
    }

    #[inline]
    fn num_fields(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn field_name_at(&self, index: usize) -> Option<&str> {
        self.field_names.get(index)
            .map(|name| name.as_ref())
    }

    #[inline]
    fn iter(&self) -> StructFieldIter {
        StructFieldIter::new(self)
    }

    fn clone_dynamic(&self) -> DynamicStruct {
        DynamicStruct {
            name: self.name.clone(),
            field_names: self.field_names.clone(),
            field_indices: self.field_indices.clone(),
            fields: self
                .fields
                .iter()
                .map(|value| value.clone_value())
                .collect(),
        }
    }
}

// implement Reflect
impl Reflect for DynamicStruct {
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
        if let ReflectRef::Struct(struct_value) = reflected.reflect_ref() {
            for (i, value) in struct_value.iter().enumerate() {
                let name = struct_value.field_name_at(i).unwrap();
                if let Some(v) = self.field_mut(name) {
                    v.assign(value);
                }
            }
        } else {
            panic!("Attempted to assign non-struct type to struct type.");
        }
    }

    fn reflect_ref<'a>(&'a self) -> ReflectRef<'a> {
        ReflectRef::Struct(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> ReflectRefMut<'a> {
        ReflectRefMut::Struct(self)
    }

    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::Struct(self)
    }

    /// #partial_eq_impl
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::struct_partial_eq(self, value)
    }

    /// #debug_impl
    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicStruct(")?;
        debug::struct_debug(self, f)?;
        write!(f, ")")
    }
}

impl Debug for DynamicStruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}