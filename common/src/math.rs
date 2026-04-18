use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const ZERO: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn dot(lhs: Vector3, rhs: Vector3) -> f32 {
        lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z
    }

    pub fn angle_between(lhs: Vector3, rhs: Vector3) -> f32 {
        Self::angle_xz(lhs.x, lhs.z, rhs.x, rhs.z)
    }

    pub fn angle_xz(x: f32, z: f32, x2: f32, z2: f32) -> f32 {
        if x == x2 {
            return 0.0;
        }
        let angle = ((z2 - z) / (x2 - x)).atan();
        if x > x2 {
            angle + std::f32::consts::PI
        } else {
            angle
        }
    }

    pub fn new_horizontal(&self, angle: f32, extents: f32) -> Vector3 {
        Vector3 {
            x: self.x + angle.cos() * extents,
            y: self.y,
            z: self.z + angle.sin() * extents,
        }
    }

    pub fn is_within_circle(&self, center: Vector3, max_radius: f32, min_radius: f32) -> bool {
        if self.x == center.x && self.z == center.z {
            return true;
        }
        let distance = crate::utils::xz_distance(center.x, center.z, self.x, self.z);
        distance <= max_radius && distance >= min_radius
    }

    pub fn is_within_box(&self, upper_left: Vector3, lower_right: Vector3) -> bool {
        upper_left.x <= self.x
            && upper_left.y <= self.y
            && upper_left.z <= self.z
            && lower_right.x >= self.x
            && lower_right.y >= self.y
            && lower_right.z >= self.z
    }

    /// Matches the legacy C# `IsWithinCone` which ignores distance.
    pub fn is_within_cone(
        &self,
        cone_center: Vector3,
        cone_rotation: f32,
        cone_angle: f32,
    ) -> bool {
        let mut angle_to_target = Self::angle_between(cone_center, *self);
        let half_angle_of_aoe = cone_angle * std::f32::consts::PI / 2.0;
        let rotation_to_add = cone_rotation + half_angle_of_aoe;

        angle_to_target = (angle_to_target + rotation_to_add - 0.5 * std::f32::consts::PI)
            % (2.0 * std::f32::consts::PI);

        angle_to_target >= 0.0 && angle_to_target <= cone_angle * std::f32::consts::PI
    }
}

impl PartialEq for Vector3 {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl Add for Vector3 {
    type Output = Vector3;
    fn add(self, rhs: Vector3) -> Vector3 {
        Vector3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vector3 {
    type Output = Vector3;
    fn sub(self, rhs: Vector3) -> Vector3 {
        Vector3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul for Vector3 {
    type Output = Vector3;
    fn mul(self, rhs: Vector3) -> Vector3 {
        Vector3::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

impl Mul<f32> for Vector3 {
    type Output = Vector3;
    fn mul(self, scalar: f32) -> Vector3 {
        Vector3::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl Mul<Vector3> for f32 {
    type Output = Vector3;
    fn mul(self, rhs: Vector3) -> Vector3 {
        rhs * self
    }
}

impl Div<f32> for Vector3 {
    type Output = Vector3;
    fn div(self, scalar: f32) -> Vector3 {
        Vector3::new(self.x / scalar, self.y / scalar, self.z / scalar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vector3::new(5.0, 7.0, 9.0));
        assert_eq!(b - a, Vector3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn length() {
        let v = Vector3::new(3.0, 0.0, 4.0);
        assert_eq!(v.length(), 5.0);
    }

    #[test]
    fn box_containment() {
        let p = Vector3::new(0.5, 0.5, 0.5);
        assert!(p.is_within_box(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0)));
        assert!(!p.is_within_box(Vector3::new(1.0, 1.0, 1.0), Vector3::new(2.0, 2.0, 2.0)));
    }
}
