mod sh;
mod aabb;

use glam::Vec3;
pub use sh::SHBasis9;

pub use aabb::AABB;

#[inline]
pub fn min_value_align_to(size: usize, alignment: usize) -> usize {
    assert_eq!(alignment.count_ones(), 1);
    (size + alignment - 1) & !(alignment - 1)
}

#[inline]
pub fn max_mipmap_level_1d(res: u32) -> u16 {
    (32 - res.leading_zeros()) as u16
}

#[inline]
pub fn max_mipmap_level_2d(width: u32, height: u32) -> u16 {
    max_mipmap_level_1d(width).max(max_mipmap_level_1d(height))
}

#[inline]
pub fn max_mipmap_level_3d(width: u32, height: u32, depth: u32) -> u16 {
    max_mipmap_level_1d(width).max(max_mipmap_level_1d(height)).max(max_mipmap_level_1d(depth))
}

#[inline]
pub fn from_rgb8_to_color(r: u8, g: u8, b: u8) -> Vec3 {
    Vec3::from_array([
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ])
}