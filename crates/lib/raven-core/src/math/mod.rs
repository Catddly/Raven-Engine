#[inline]
pub fn min_value_align_to(size: usize, alignment: usize) -> usize {
    assert_eq!(alignment.count_ones(), 1);
    (size + alignment - 1) & !(alignment - 1)
}