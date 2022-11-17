use std::f32::consts::PI;

use glam::Vec3;

#[derive(Debug, Clone)]
pub struct SHBasis9 {
    y00  : f32,
    y1_1 : f32,
    y10  : f32,
    y11  : f32,
    y2_2 : f32,
    y2_1 : f32,
    y20  : f32,
    y21  : f32,
    y22  : f32
}

impl SHBasis9 {
    pub fn zero() -> SHBasis9 {
        Self {
            y00  : 0.0,
            y1_1 : 0.0,
            y10  : 0.0,
            y11  : 0.0,
            y2_2 : 0.0,
            y2_1 : 0.0,
            y20  : 0.0,
            y21  : 0.0,
            y22  : 0.0,
        }
    }

    pub fn coefficients() -> SHBasis9 {
        let y1n = 0.5 * (3.0 / PI).sqrt();
        let y2n = 0.5 * (15.0 / PI).sqrt();

        Self {
            y00  : 0.5 * (1.0 / PI).sqrt(),
            y1_1 : y1n,
            y10  : y1n,
            y11  : y1n,
            y2_2 : y2n,
            y2_1 : y2n,
            y20  : 0.25 * (5.0 / PI).sqrt(),
            y21  : y2n,
            y22  : 0.25 * (15.0 / PI).sqrt(),
        }
    }

    pub fn from_direction_cartesian(direction: Vec3) -> SHBasis9 {
        assert!(direction.is_normalized());

        let coefficients = Self::coefficients();

        // assume that r = 1.0
        let dir_basis = SHBasis9 {
            y00  : 1.0,
            y1_1 : direction.y,
            y10  : direction.z,
            y11  : direction.x,
            y2_2 : direction.y * direction.x,
            y2_1 : direction.y * direction.z,
            //y20  : (2.0 * direction.z * direction.z) - (direction.x * direction.x) - (direction.y * direction.y),
            y20  : (3.0 * direction.z * direction.z) - 1.0,
            y21  : direction.z * direction.x,
            y22  : (direction.x * direction.x) - (direction.y * direction.y)
        };

        coefficients.mul_basis(&dir_basis)
    }

    pub fn mul_basis(&self, basis: &SHBasis9) -> SHBasis9 {
        SHBasis9 {
            y00  : self.y00 * basis.y00,
            y1_1 : self.y1_1 * basis.y1_1,
            y10  : self.y10 * basis.y10,
            y11  : self.y11 * basis.y11,
            y2_2 : self.y2_2 * basis.y2_2,
            y2_1 : self.y2_1 * basis.y2_1,
            y20  : self.y20 * basis.y20,
            y21  : self.y21 * basis.y21,
            y22  : self.y22 * basis.y22
        }
    }

    pub fn add(&self, basis: &SHBasis9) -> SHBasis9 {
        SHBasis9 {
            y00  : self.y00 + basis.y00,
            y1_1 : self.y1_1 + basis.y1_1,
            y10  : self.y10 + basis.y10,
            y11  : self.y11 + basis.y11,
            y2_2 : self.y2_2 + basis.y2_2,
            y2_1 : self.y2_1 + basis.y2_1,
            y20  : self.y20 + basis.y20,
            y21  : self.y21 + basis.y21,
            y22  : self.y22 + basis.y22
        }
    }

    pub fn mul_scaler(&self, scaler: f32) -> SHBasis9 {
        SHBasis9 {
            y00  : self.y00  * scaler,
            y1_1 : self.y1_1 * scaler,
            y10  : self.y10  * scaler,
            y11  : self.y11  * scaler,
            y2_2 : self.y2_2 * scaler,
            y2_1 : self.y2_1 * scaler,
            y20  : self.y20  * scaler,
            y21  : self.y21  * scaler,
            y22  : self.y22  * scaler,   
        }
    }

    pub fn to_f32_array(&self) -> [f32; 9] {
        [
            self.y00,
            self.y1_1,
            self.y10,
            self.y11, 
            self.y2_2, 
            self.y2_1,
            self.y20,
            self.y21, 
            self.y22
        ]
    }
}