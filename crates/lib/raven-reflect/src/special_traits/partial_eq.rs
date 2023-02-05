use crate::{type_info::Struct, Reflect, ReflectRef, Tuple, Enum, VariantForm, Map, TupleStruct, Array, List};

/// partial equivalent comparison for Struct reflect type.
/// 
/// All conditions should pass to partial eq:
/// 
/// * The Reflect type should be the same.
/// * The number of fields should be the same.
/// * Name and value of each field should correspond to each other one by one.
#[inline]
pub fn struct_partial_eq<S: Struct>(lhs: &S, rhs: &dyn Reflect) -> Option<bool> {
    let ReflectRef::Struct(struct_value) = rhs.reflect_ref() else {
        return Some(false);
    };

    if lhs.num_fields() != struct_value.num_fields() {
        return Some(false);
    }

    for (i, value) in struct_value.iter().enumerate() {
        let name = struct_value.field_name_at(i).unwrap();

        if let Some(field_value) = lhs.field(name) {
            let eq_result = field_value.reflect_partial_eq(value);
            if let failed @ (Some(false) | None) = eq_result {
                return failed;
            }
        } else {
            return Some(false);
        }
    }

    Some(true)
}

#[inline]
pub fn tuple_partial_eq<S: Tuple>(lhs: &S, rhs: &dyn Reflect) -> Option<bool> {
    let ReflectRef::Tuple(tuple_value) = rhs.reflect_ref() else {
        return Some(false);
    };

    if lhs.num_fields() != tuple_value.num_fields() {
        return Some(false);
    }

    for (lhs_field, rhs_field) in lhs.iter().zip(tuple_value.iter()) {
        let eq_result = lhs_field.reflect_partial_eq(rhs_field);
        if let failed @ (Some(false) | None) = eq_result {
            return failed;
        }
    }

    Some(true)
}

#[inline]
pub fn enum_partial_eq<S: Enum>(lhs: &S, rhs: &dyn Reflect) -> Option<bool> {
    let ReflectRef::Enum(rhs) = rhs.reflect_ref() else {
        return Some(false);
    };

    if lhs.variant_name() != rhs.variant_name() {
        return Some(false);
    }

    if !lhs.is_form(rhs.variant_form()) {
        return Some(false);
    }

    match lhs.variant_form() {
        VariantForm::Struct => {
            // Same struct fields?
            for field in lhs.iter() {
                let field_name = field.name().unwrap();
                if let Some(field_value) = rhs.field(field_name) {
                    if let Some(false) | None = field_value.reflect_partial_eq(field.value()) {
                        // Fields failed comparison
                        return Some(false);
                    }
                } else {
                    // Field does not exist
                    return Some(false);
                }
            }
            Some(true)
        }
        VariantForm::Tuple => {
            // Same tuple fields?
            for (i, field) in lhs.iter().enumerate() {
                if let Some(field_value) = rhs.field_at(i) {
                    if let Some(false) | None = field_value.reflect_partial_eq(field.value()) {
                        // Fields failed comparison
                        return Some(false);
                    }
                } else {
                    // Field does not exist
                    return Some(false);
                }
            }
            Some(true)
        }
        _ => Some(true),
    }
}

#[inline]
pub fn map_partial_eq<M: Map>(a: &M, b: &dyn Reflect) -> Option<bool> {
    let ReflectRef::Map(map) = b.reflect_ref() else {
        return Some(false);
    };

    if a.len() != map.len() {
        return Some(false);
    }

    for (key, value) in a.iter() {
        if let Some(map_value) = map.get(key) {
            let eq_result = value.reflect_partial_eq(map_value);
            if let failed @ (Some(false) | None) = eq_result {
                return failed;
            }
        } else {
            return Some(false);
        }
    }

    Some(true)
}

#[inline]
pub fn tuple_struct_partial_eq<T: TupleStruct>(a: &T, b: &dyn Reflect) -> Option<bool> {
    let ReflectRef::TupleStruct(tuple_struct) = b.reflect_ref() else {
        return Some(false);
    };

    if a.num_fields() != tuple_struct.num_fields() {
        return Some(false);
    }

    for (i, value) in tuple_struct.iter().enumerate() {
        if let Some(field_value) = a.field_at(i) {
            let eq_result = field_value.reflect_partial_eq(value);
            if let failed @ (Some(false) | None) = eq_result {
                return failed;
            }
        } else {
            return Some(false);
        }
    }

    Some(true)
}

#[inline]
pub fn array_partial_eq<A: Array>(array: &A, reflect: &dyn Reflect) -> Option<bool> {
    match reflect.reflect_ref() {
        ReflectRef::Array(reflect_array) if reflect_array.len() == array.len() => {
            for (a, b) in array.iter().zip(reflect_array.iter()) {
                let eq_result = a.reflect_partial_eq(b);
                if let failed @ (Some(false) | None) = eq_result {
                    return failed;
                }
            }
        }
        _ => return Some(false),
    }

    Some(true)
}

#[inline]
pub fn list_partial_eq<L: List>(a: &L, b: &dyn Reflect) -> Option<bool> {
    let ReflectRef::List(list) = b.reflect_ref() else {
        return Some(false);
    };

    if a.len() != list.len() {
        return Some(false);
    }

    for (a_value, b_value) in a.iter().zip(list.iter()) {
        let eq_result = a_value.reflect_partial_eq(b_value);
        if let failed @ (Some(false) | None) = eq_result {
            return failed;
        }
    }

    Some(true)
}