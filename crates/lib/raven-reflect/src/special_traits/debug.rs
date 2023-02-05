use std::fmt::Debug;

use crate::{type_info::Struct, Tuple, Enum, VariantForm, Map, TupleStruct, Array, List};

#[inline]
pub fn struct_debug(dyn_struct: &dyn Struct, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_struct(dyn_struct.type_name());
    for field_index in 0..dyn_struct.num_fields() {
        let field = dyn_struct.field_at(field_index).unwrap();
        debug.field(
            dyn_struct.field_name_at(field_index).unwrap(),
            &field as &dyn Debug,
        );
    }
    debug.finish()
}

#[inline]
pub fn tuple_debug(dyn_tuple: &dyn Tuple, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_tuple(dyn_tuple.type_name());
    for field_index in 0..dyn_tuple.num_fields() {
        let field = dyn_tuple.field_at(field_index).unwrap();
        debug.field(&field as &dyn Debug);
    }
    debug.finish()
}

#[inline]
pub fn enum_debug(dyn_enum: &dyn Enum, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match dyn_enum.variant_form() {
        VariantForm::Unit => f.write_str(dyn_enum.variant_name()),
        VariantForm::Tuple => {
            let mut debug = f.debug_tuple(dyn_enum.variant_name());
            for field in dyn_enum.iter() {
                debug.field(&field.value() as &dyn Debug);
            }
            debug.finish()
        }
        VariantForm::Struct => {
            let mut debug = f.debug_struct(dyn_enum.variant_name());
            for field in dyn_enum.iter() {
                debug.field(field.name().unwrap(), &field.value() as &dyn Debug);
            }
            debug.finish()
        }
    }
}

#[inline]
pub fn map_debug(dyn_map: &dyn Map, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_map();
    for (key, value) in dyn_map.iter() {
        debug.entry(&key as &dyn Debug, &value as &dyn Debug);
    }
    debug.finish()
}

#[inline]
pub fn tuple_struct_debug(
    dyn_tuple_struct: &dyn TupleStruct,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let mut debug = f.debug_tuple(dyn_tuple_struct.type_name());
    for field in dyn_tuple_struct.iter() {
        debug.field(&field as &dyn Debug);
    }
    debug.finish()
}

#[inline]
pub fn array_debug(dyn_array: &dyn Array, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_list();
    for item in dyn_array.iter() {
        debug.entry(&item as &dyn Debug);
    }
    debug.finish()
}

#[inline]
pub fn list_debug(dyn_list: &dyn List, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = f.debug_list();
    for item in dyn_list.iter() {
        debug.entry(&item as &dyn Debug);
    }
    debug.finish()
}
