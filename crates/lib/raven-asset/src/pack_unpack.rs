use std::marker::PhantomData;

use byteorder::{WriteBytesExt};

#[derive(Debug, Copy, Clone)]
pub struct FlatVec<T> {
    len: u64,
    // offset from current address (not from the start of the byte buffer)
    offset: u64,
    // invariant
    _marker: PhantomData<T>,
}

/// Pack a plain field of struct to raw bytes.
pub(super) fn pack_plain_field<T: Sized + Copy>(writer: &mut impl std::io::Write, field: &T) {
    let byte_ptr = unsafe { std::slice::from_raw_parts(
        field as *const T as *const u8, 
        std::mem::size_of::<T>()
    )};

    writer.write_all(byte_ptr).unwrap();
}

#[allow(dead_code)]
pub(super) fn pack_bytes(writer: &mut impl std::io::Write, bytes: &[u8]) {
    writer.write_all(bytes).unwrap();
}

/// Pack a field of Vec type in the struct to raw bytes.
pub(super) fn pack_vec_header(writer: &mut Vec<u8>, len: u64) -> usize {
    // len in FlatVec
    writer.write_u64::<byteorder::NativeEndian>(len).unwrap();
    // offset in FlatVec
    writer.write_u64::<byteorder::NativeEndian>(0).unwrap();
    // the offset u64 position
    writer.len() - 8
}

/// Unpack FlatVec from raw bytes.
pub(super) fn read_flat_vec<'a, T: Sized + Copy>(flat_vec_addr: *const FlatVec<T>) -> &'a [T] {
    unsafe {
        let offset_addr = (flat_vec_addr as *const u8).add(std::mem::size_of::<u64>());
        let data = offset_addr.add((*flat_vec_addr).offset as usize);
        std::slice::from_raw_parts(data as *const T, (*flat_vec_addr).len as usize)
    }
}
