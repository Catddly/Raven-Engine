use crate::{DynamicTuple, DynamicStruct, Tuple, Struct};

/// A enum variant which allow variant form to be modified at runtime.
#[derive(Debug, Default)]
pub enum DynamicVariant {
    #[default]
    Unit,
    Tuple(DynamicTuple),
    Struct(DynamicStruct),
}

impl Clone for DynamicVariant {
    fn clone(&self) -> Self {
        match self {
            DynamicVariant::Unit => DynamicVariant::Unit,
            DynamicVariant::Tuple(data) => DynamicVariant::Tuple(data.clone_dynamic()),
            DynamicVariant::Struct(data) => DynamicVariant::Struct(data.clone_dynamic()),
        }
    }
}

impl From<DynamicTuple> for DynamicVariant {
    fn from(dyn_tuple: DynamicTuple) -> Self {
        Self::Tuple(dyn_tuple)
    }
}

impl From<DynamicStruct> for DynamicVariant {
    fn from(dyn_struct: DynamicStruct) -> Self {
        Self::Struct(dyn_struct)
    }
}

impl From<()> for DynamicVariant {
    fn from(_: ()) -> Self {
        Self::Unit
    }
}