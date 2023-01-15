use super::{Vec3, Mat4};

/// Axis-Aligned Bounding Box
#[derive(Copy, Clone, Debug)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3
}

impl AABB {
    pub fn new() -> Self {
        Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN)
        }
    }

    pub fn from_min_max(min: Vec3, max: Vec3) -> Self {
        Self {
            min,
            max
        }
    }

    pub fn reset(&mut self) {
        self.min = Vec3::splat(f32::MAX);
        self.max = Vec3::splat(f32::MIN);
    }

    pub fn is_valid(&self) -> bool {
        self.min.cmplt(self.max).all()
    }

    pub fn merge_aabb(&mut self, other: &AABB) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    pub fn merge_point_vec3(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    pub fn merge_point_f32(&mut self, point: &[f32; 3]) {
        self.min = Vec3::from((
            self.min.x.min(point[0]),
            self.min.y.min(point[1]),
            self.min.z.min(point[2])
        ));
        self.max = Vec3::from((
            self.max.x.max(point[0]),
            self.max.y.max(point[1]),
            self.max.z.max(point[2])
        ));
    }

    pub fn transform(&mut self, matrix: Mat4) {
        // TODO: optimize it
        let corners: [Vec3; 8] = [
			self.min,
			Vec3::from((self.min.x, self.min.y, self.max.z)),
			Vec3::from((self.min.x, self.max.y, self.min.z)),
			Vec3::from((self.max.x, self.min.y, self.min.z)),
			Vec3::from((self.min.x, self.max.y, self.max.z)),
			Vec3::from((self.max.x, self.min.y, self.max.z)),
			Vec3::from((self.max.x, self.max.y, self.min.z)),
			self.max
        ];

        self.reset();

        for i in 0..8 {
            let transformed_corner = matrix * corners[i].extend(1.0);
            let transformed_corner = transformed_corner.truncate() / transformed_corner.w;
            self.merge_point_vec3(transformed_corner);
        }
    }

    /// Return the center of the AABB.
    pub fn get_center(&self) -> Vec3 {
        debug_assert!(self.is_valid());

        (self.min + self.max) * 0.5
    }

    /// Return the extent of the AABB (half the length to the axis border).
    pub fn get_extent(&self) -> Vec3 {
        debug_assert!(self.is_valid());

        (self.max - self.min) * 0.5
    }
}