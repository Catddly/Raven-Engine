mod sh;

#[inline]
pub fn min_value_align_to(size: usize, alignment: usize) -> usize {
    assert_eq!(alignment.count_ones(), 1);
    (size + alignment - 1) & !(alignment - 1)
}

#[inline]
pub fn max_mipmap_level(width: u32, height: u32) -> u16 {
    (32 - width.leading_zeros()).max(32 - height.leading_zeros()) as u16
}

#[inline]
pub fn from_rgb8_to_color(r: u8, g: u8, b: u8) -> Vec3 {
    Vec3::from_array([
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ])
}

use glam::Vec3;
pub use sh::SHBasis9;