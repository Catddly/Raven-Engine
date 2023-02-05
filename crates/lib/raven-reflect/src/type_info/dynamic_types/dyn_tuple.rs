use std::fmt::Formatter;

use crate::{Reflect, Typed, TypeInfo, NonGenericTypeInfoOnceCell, DynamicTypeInfo, Tuple, TupleFieldIter, ReflectRef, ReflectRefMut, ReflectOwned, special_traits::{partial_eq, debug}};

/// A tuple which allow fields to be modified at runtime.
#[derive(Default, Debug)]
pub struct DynamicTuple {
    name: String,
    fields: Vec<Box<dyn Reflect>>,
}

impl DynamicTuple {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    /// Appends an element with value `value` to the tuple.
    pub fn add_field_boxed(&mut self, value: Box<dyn Reflect>) {
        self.fields.push(value);
        self.generate_name();
    }

    /// Appends a typed element with value `value` to the tuple.
    pub fn add_field<T: Reflect>(&mut self, value: T) {
        self.add_field_boxed(Box::new(value));
        self.generate_name();
    }

    fn generate_name(&mut self) {
        let name = &mut self.name;
        name.clear();
        name.push('(');
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                name.push_str(", ");
            }
            name.push_str(field.type_name());
        }
        name.push(')');
    }
}

// implement Typed
impl Typed for DynamicTuple {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(
            DynamicTypeInfo::new::<Self>()
        ))
    }
}

// implement Tuple
impl Tuple for DynamicTuple {
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
    fn iter(&self) -> TupleFieldIter {
        TupleFieldIter::new(self)
    }

    #[inline]
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.fields
    }

    fn clone_dynamic(&self) -> DynamicTuple {
        DynamicTuple {
            name: self.name.clone(),
            fields: self
                .fields
                .iter()
                .map(|value| value.clone_value())
                .collect(),
        }
    }
}

// implement Reflect
impl Reflect for DynamicTuple {
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
        if let ReflectRef::Tuple(tuple_value) = reflected.reflect_ref() {
            for (i, value) in tuple_value.iter().enumerate() {
                if let Some(v) = self.field_at_mut(i) {
                    v.assign(value);
                }
            }
        } else {
            panic!("Attempted to assign non-tuple type to tuple type.");
        }
    }

    fn reflect_ref<'a>(&'a self) -> ReflectRef<'a> {
        ReflectRef::Tuple(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> ReflectRefMut<'a> {
        ReflectRefMut::Tuple(self)
    }

    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::Tuple(self)
    }

    /// #partial_eq_impl
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::tuple_partial_eq(self, value)
    }

    /// #debug_impl
    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicTuple(")?;
        debug::tuple_debug(self, f)?;
        write!(f, ")")
    }
}