#[inline]
pub fn min_value_align_to(size: usize, alignment: usize) -> usize {
    assert_eq!(alignment.count_ones(), 1);
    (size + alignment - 1) & !(alignment - 1)
}

#[inline]
pub fn max_mipmap_level(width: u32, height: u32) -> u16 {
    (32 - width.leading_zeros()).max(32 - height.leading_zeros()) as u16
}