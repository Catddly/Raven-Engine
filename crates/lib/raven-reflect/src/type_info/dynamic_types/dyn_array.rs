use crate::{Reflect, TypeInfo, Typed, NonGenericTypeInfoOnceCell, DynamicTypeInfo, Array, ReflectRef, special_traits::{partial_eq, debug}};

pub struct DynamicArray {
    name: String,
    values: Box<[Box<dyn Reflect>]>,
}

impl DynamicArray {
    pub fn new(values: Box<[Box<dyn Reflect>]>) -> Self {
        Self {
            name: String::default(),
            values,
        }
    }

    pub fn from_vec<T: Reflect>(values: Vec<T>) -> Self {
        Self {
            name: String::default(),
            values: values
                .into_iter()
                .map(|field| Box::new(field) as Box<dyn Reflect>)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

// impl Typed
impl Typed for DynamicArray {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(DynamicTypeInfo::new::<Self>()))
    }
}

// impl Debug
impl std::fmt::Debug for DynamicArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

// impl Array
impl Array for DynamicArray {
    #[inline]
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        self.values.get(index).map(|item| &**item)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.values.get_mut(index).map(|item| &mut **item)
    }

    #[inline]
    fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn iter(&self) -> crate::ArrayIter {
        crate::ArrayIter::new(self)
    }

    #[inline]
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.values.into_vec()
    }

    fn clone_dynamic(&self) -> DynamicArray {
        DynamicArray { 
            name: self.name.clone(),
            values: self.values
                .iter()
                .map(|v| v.clone_value())
                .collect()
        }
    }
}

// impl Reflect
impl Reflect for DynamicArray {
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

    fn assign(&mut self, reflected: &dyn Reflect) {
        if let ReflectRef::Array(reflect_array) = reflected.reflect_ref() {
            if self.len() != reflect_array.len() {
                panic!("Attempted to apply different sized `Array` types.");
            }
            for (i, value) in reflect_array.iter().enumerate() {
                let v = self.get_mut(i).unwrap();
                v.assign(value);
            }
        } else {
            panic!("Attempted to apply a non-`Array` type to an `Array` type.");
        }
    }
    
    fn reflect_ref<'a>(&'a self) -> crate::ReflectRef<'a> {
        crate::ReflectRef::Array(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> crate::ReflectRefMut<'a> {
        crate::ReflectRefMut::Array(self)
    }

    fn reflect_owned<'a>(self: Box<Self>) -> crate::ReflectOwned {
        crate::ReflectOwned::Array(self)
    }

    /// #partial_eq_impl
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::array_partial_eq(self, value)
    }

    /// #debug_impl
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicArray(")?;
        debug::array_debug(self, f)?;
        write!(f, ")")
    }
}