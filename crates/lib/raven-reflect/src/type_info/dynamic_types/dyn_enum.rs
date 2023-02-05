use std::fmt::Formatter;

use crate::{DynamicVariant, Enum, VariantForm, DynamicTuple, DynamicStruct, Struct, Reflect, VariantFieldIter, Tuple, Typed, TypeInfo, NonGenericTypeInfoOnceCell, DynamicTypeInfo, ReflectRef, ReflectRefMut, ReflectOwned, special_traits::{partial_eq, debug}};

/// A enum which allow variants to be modified at runtime.
#[derive(Default)]
pub struct DynamicEnum {
    name: String,
    variant_name: String,
    variant_index: usize,
    variant: DynamicVariant,
}

impl DynamicEnum {
    pub fn new<I: Into<String>, V: Into<DynamicVariant>>(
        name: I,
        variant_name: I,
        variant: V,
    ) -> Self {
        Self {
            name: name.into(),
            variant_index: 0,
            variant_name: variant_name.into(),
            variant: variant.into(),
        }
    }

    pub fn new_with_index<I: Into<String>, V: Into<DynamicVariant>>(
        name: I,
        variant_index: usize,
        variant_name: I,
        variant: V,
    ) -> Self {
        Self {
            name: name.into(),
            variant_index,
            variant_name: variant_name.into(),
            variant: variant.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_variant<I: Into<String>, V: Into<DynamicVariant>>(&mut self, name: I, variant: V) {
        self.variant_name = name.into();
        self.variant = variant.into();
    }

    pub fn set_variant_with_index<I: Into<String>, V: Into<DynamicVariant>>(
        &mut self,
        variant_index: usize,
        name: I,
        variant: V,
    ) {
        self.variant_index = variant_index;
        self.variant_name = name.into();
        self.variant = variant.into();
    }

    pub fn from<T: Enum>(value: T) -> Self {
        Self::from_ref(&value)
    }

    pub fn from_ref<T: Enum>(value: &T) -> Self {
        match value.variant_form() {
            VariantForm::Unit => DynamicEnum::new_with_index(
                value.type_name(),
                value.variant_index(),
                value.variant_name(),
                DynamicVariant::Unit,
            ),
            VariantForm::Tuple => {
                let mut data = DynamicTuple::default();
                for field in value.iter() {
                    data.add_field_boxed(field.value().clone_value());
                }
                DynamicEnum::new_with_index(
                    value.type_name(),
                    value.variant_index(),
                    value.variant_name(),
                    DynamicVariant::Tuple(data),
                )
            }
            VariantForm::Struct => {
                let mut data = DynamicStruct::default();
                for field in value.iter() {
                    let name = field.name().unwrap();
                    data.add_field_boxed(name, field.value().clone_value());
                }
                DynamicEnum::new_with_index(
                    value.type_name(),
                    value.variant_index(),
                    value.variant_name(),
                    DynamicVariant::Struct(data),
                )
            }
        }
    }
}

// impl Typed
impl Typed for DynamicEnum {
    fn type_info() -> &'static TypeInfo {
        static TYPE_INFO_CELL: NonGenericTypeInfoOnceCell = NonGenericTypeInfoOnceCell::new();
        TYPE_INFO_CELL.get_or_set(|| TypeInfo::Dynamic(DynamicTypeInfo::new::<Self>()))
    }
}

// impl Debug
impl std::fmt::Debug for DynamicEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

// impl Enum
impl Enum for DynamicEnum {
    fn field(&self, name: &str) -> Option<&dyn Reflect> {
        if let DynamicVariant::Struct(data) = &self.variant {
            data.field(name)
        } else {
            None
        }
    }

    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect> {
        if let DynamicVariant::Struct(data) = &mut self.variant {
            data.field_mut(name)
        } else {
            None
        }
    }
    
    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        if let DynamicVariant::Tuple(data) = &self.variant {
            data.field_at(index)
        } else {
            None
        }
    }

    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        if let DynamicVariant::Tuple(data) = &mut self.variant {
            data.field_at_mut(index)
        } else {
            None
        }
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        if let DynamicVariant::Struct(data) = &self.variant {
            data.index_of(name)
        } else {
            None
        }
    }

    fn field_name_at(&self, index: usize) -> Option<&str> {
        if let DynamicVariant::Struct(data) = &self.variant {
            data.field_name_at(index)
        } else {
            None
        }
    }

    fn iter(&self) -> VariantFieldIter {
        VariantFieldIter::new(self)
    }

    fn num_fields(&self) -> usize {
        match &self.variant {
            DynamicVariant::Unit => 0,
            DynamicVariant::Tuple(data) => data.num_fields(),
            DynamicVariant::Struct(data) => data.num_fields(),
        }
    }
    
    fn variant_name(&self) -> &str {
        &self.variant_name
    }

    fn variant_index(&self) -> usize {
        self.variant_index
    }

    fn variant_form(&self) -> VariantForm {
        match &self.variant {
            DynamicVariant::Unit => VariantForm::Unit,
            DynamicVariant::Tuple(..) => VariantForm::Tuple,
            DynamicVariant::Struct(..) => VariantForm::Struct,
        }
    }

    fn clone_dynamic(&self) -> DynamicEnum {
        Self {
            name: self.name.clone(),
            variant_index: self.variant_index,
            variant_name: self.variant_name.clone(),
            variant: self.variant.clone(),
        }
    }
}

// implement Reflect
impl Reflect for DynamicEnum {
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
        if let ReflectRef::Enum(value) = reflected.reflect_ref() {
            if Enum::variant_name(self) == value.variant_name() {
                // same variant -> just update fields
                match value.variant_form() {
                    VariantForm::Struct => {
                        for field in value.iter() {
                            let name = field.name().unwrap();
                            if let Some(v) = Enum::field_mut(self, name) {
                                v.assign(field.value());
                            }
                        }
                    }
                    VariantForm::Tuple => {
                        for (index, field) in value.iter().enumerate() {
                            if let Some(v) = Enum::field_at_mut(self, index) {
                                v.assign(field.value());
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                // different variant -> perform a switch
                let dyn_variant = match value.variant_form() {
                    VariantForm::Unit => DynamicVariant::Unit,
                    VariantForm::Tuple => {
                        let mut dyn_tuple = DynamicTuple::default();
                        for field in value.iter() {
                            dyn_tuple.add_field_boxed(field.value().clone_value());
                        }
                        DynamicVariant::Tuple(dyn_tuple)
                    }
                    VariantForm::Struct => {
                        let mut dyn_struct = DynamicStruct::default();
                        for field in value.iter() {
                            dyn_struct.add_field_boxed(field.name().unwrap(), field.value().clone_value());
                        }
                        DynamicVariant::Struct(dyn_struct)
                    }
                };
                self.set_variant(value.variant_name(), dyn_variant);
            }
        } else {
            panic!("`{}` is not an enum", reflected.type_name());
        }
    }

    fn reflect_ref<'a>(&'a self) -> ReflectRef<'a> {
        ReflectRef::Enum(self)
    }

    fn reflect_ref_mut<'a>(&'a mut self) -> ReflectRefMut<'a> {
        ReflectRefMut::Enum(self)
    }

    fn reflect_owned(self: Box<Self>) -> ReflectOwned {
        ReflectOwned::Enum(self)
    }

    /// #partial_eq_impl
    fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
        partial_eq::enum_partial_eq(self, value)
    }

    /// #debug_impl
    fn debug(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicEnum(")?;
        debug::enum_debug(self, f)?;
        write!(f, ")")
    }
}