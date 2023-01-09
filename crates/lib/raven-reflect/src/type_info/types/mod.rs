mod field;

mod primitive_type;
mod array_type;

mod struct_type;

pub use field::{NamedField, UnnamedFiled};

pub use primitive_type::{PrimitiveTypeInfo};
pub use array_type::{ArrayTypeInfo, ArrayIter};

pub use struct_type::{StructTypeInfo, FieldIter, Struct};
