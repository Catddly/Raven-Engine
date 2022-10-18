pub fn as_byte_slice<T: Copy>(value: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
    }
}

pub fn as_byte_slice_values<T: Copy>(values: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(values as *const T as *const u8, std::mem::size_of_val(&values))
    }
}