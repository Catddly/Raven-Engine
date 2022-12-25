use glam::Vec3;

/// Axis-Aligned Bounding Box
#[derive(Copy, Clone, Debug)]
pub struct AABB {
    min: Vec3,
    max: Vec3
}

impl AABB {
    pub fn new() -> Self {
        Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN)
        }
    }

    pub fn is_valid(&self) -> bool {
        self.min.cmplt(self.max).all()
    }

    pub fn merge_aabb(&mut self, other: AABB) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    pub fn merge_point(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    } 
}