use crate::{type_info::types::enum_type::Enum, Reflect};

use super::VariantForm;

pub struct VariantFieldIter<'a> {
    pub(crate) refl_enum: &'a dyn Enum,
    pub(crate) curr_index: usize,
}

impl<'a> VariantFieldIter<'a> {
    pub fn new(refl_enum: &'a dyn Enum) -> Self {
        Self {
            refl_enum,
            curr_index: 0,
        }
    }
}

impl<'a> Iterator for VariantFieldIter<'a> {
    type Item = VariantField<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let field = match self.refl_enum.variant_form() {
            VariantForm::Unit => None,
            VariantForm::Tuple => Some(VariantField::Tuple(self.refl_enum.field_at(self.curr_index)?)),
            VariantForm::Struct => {
                let name = self.refl_enum.field_name_at(self.curr_index)?;
                Some(VariantField::Struct(name, self.refl_enum.field(name)?))
            }
        };
        self.curr_index += 1;
        field
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Note: we already know the number of fields at compile-time reflected data.
        let field_len = self.refl_enum.num_fields();
        (field_len, Some(field_len))
    }
}

impl<'a> ExactSizeIterator for VariantFieldIter<'a> {}

pub enum VariantField<'a> {
    Struct(&'a str, &'a dyn Reflect),
    Tuple(&'a dyn Reflect)
}

impl<'a> VariantField<'a> {
    pub fn name(&self) -> Option<&'a str> {
        if let Self::Struct(name, ..) = self {
            Some(*name)
        } else {
            None
        }
    }

    pub fn value(&self) -> &'a dyn Reflect {
        match self {
            Self::Struct(.., value) | Self::Tuple(value) => *value,
        }
    }
}
