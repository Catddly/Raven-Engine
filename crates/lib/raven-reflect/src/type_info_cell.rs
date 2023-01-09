use std::{collections::HashMap, any::{TypeId, Any}};

use once_cell::{race::OnceBox};
use parking_lot::RwLock;

use crate::type_info::TypeInfo;

/// From bevy_reflect::utility
///
/// TypeInfo container to store reflected TypeInfo from macro.
/// 
/// Use a OnceCell to make TypeInfo has static lifetime.
pub struct NonGenericTypeInfoOnceCell(OnceBox<TypeInfo>);

impl NonGenericTypeInfoOnceCell {
    pub const fn new() -> Self {
        Self(OnceBox::new())
    }

    pub fn get_or_set<F>(&self, func: F) -> &TypeInfo
    where
        F: FnOnce() -> TypeInfo,
    {
        self.0.get_or_init(|| Box::new(func()))
    }
}

/// From bevy_reflect::utility
pub struct GenericTypeInfoOnceCell(OnceBox<RwLock<HashMap<TypeId, &'static TypeInfo>>>);

impl GenericTypeInfoOnceCell {
    pub const fn new() -> Self {
        Self(OnceBox::new())
    }

    pub fn get_or_insert<T, F>(&self, func: F) -> &TypeInfo
    where
        F: FnOnce() -> TypeInfo,
        T: Any + ?Sized
    {
        let type_id = TypeId::of::<T>();
        let map = self.0.get_or_init(Box::default);
        // already cached for this generic type, return it.
        if let Some(info) = map.read().get(&type_id) {
            return info;
        }

        // found a new generic type, insert it into the map.
        map.write().entry(type_id).or_insert_with(|| {
            // We leak here in order to obtain a `&'static` reference.
            // Otherwise, we won't be able to return a reference due to the `RwLock`.
            // This should be okay, though, since we expect it to remain statically
            // available over the course of the application.
            Box::leak(Box::new(func()))
        })
    }
}

/// From bevy_utils::short_names.rs
///
/// Shortens a type name to remove all module paths.
///
/// The short name of a type is its full name as returned by
/// [`std::any::type_name`], but with the prefix of all paths removed. For
/// example, the short name of `alloc::vec::Vec<core::option::Option<u32>>`
/// would be `Vec<Option<u32>>`.
pub fn get_type_collapsed_name(full_name: &str) -> String {
    // Generics result in nested paths within <..> blocks.
    // Consider "bevy_render::camera::camera::extract_cameras<bevy_render::camera::bundle::Camera3d>".
    // To tackle this, we parse the string from left to right, collapsing as we go.
    let mut index: usize = 0;
    let end_of_string = full_name.len();
    let mut parsed_name = String::new();

    while index < end_of_string {
        let rest_of_string = full_name.get(index..end_of_string).unwrap_or_default();

        // Collapse everything up to the next special character,
        // then skip over it
        if let Some(special_character_index) = rest_of_string.find(|c: char| {
            (c == ' ')
                || (c == '<')
                || (c == '>')
                || (c == '(')
                || (c == ')')
                || (c == '[')
                || (c == ']')
                || (c == ',')
                || (c == ';')
        }) {
            let segment_to_collapse = rest_of_string
                .get(0..special_character_index)
                .unwrap_or_default();
            parsed_name += collapse_type_name(segment_to_collapse);
            // Insert the special character
            let special_character =
                &rest_of_string[special_character_index..=special_character_index];
            parsed_name.push_str(special_character);
            // Move the index just past the special character
            index += special_character_index + 1;
        } else {
            // If there are no special characters left, we're done!
            parsed_name += collapse_type_name(rest_of_string);
            index = end_of_string;
        }
    }
    parsed_name
}

#[inline(always)]
fn collapse_type_name(string: &str) -> &str {
    string.split("::").last().unwrap()
}