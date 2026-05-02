#[derive(Clone, Copy, Debug)]
struct Plane {
    normal: glam::Vec3,
    d: f32,
}

impl Plane {
    fn normalize(self) -> Self {
        let len = self.normal.length();
        if len <= f32::EPSILON {
            return self;
        }
        Self {
            normal: self.normal / len,
            d: self.d / len,
        }
    }

    fn distance(self, p: glam::Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub fn from_view_proj(view_proj: glam::Mat4) -> Self {
        let m = view_proj.to_cols_array_2d();
        let row = |i: usize| glam::Vec4::new(m[0][i], m[1][i], m[2][i], m[3][i]);
        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);
        let make = |v: glam::Vec4| {
            Plane {
                normal: glam::Vec3::new(v.x, v.y, v.z),
                d: v.w,
            }
            .normalize()
        };

        Self {
            planes: [
                make(r3 + r0),
                make(r3 - r0),
                make(r3 + r1),
                make(r3 - r1),
                make(r2),
                make(r3 - r2),
            ],
        }
    }

    pub fn intersects_aabb(&self, min: glam::Vec3, max: glam::Vec3) -> bool {
        for plane in self.planes {
            let positive = glam::Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );
            if plane.distance(positive) < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_frustum() -> Frustum {
        let view = glam::Mat4::look_at_rh(
            glam::Vec3::ZERO,
            glam::Vec3::new(0.0, 0.0, -1.0),
            glam::Vec3::Y,
        );
        let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        Frustum::from_view_proj(proj * view)
    }

    #[test]
    fn aabb_inside_frustum_intersects() {
        let f = test_frustum();
        assert!(f.intersects_aabb(
            glam::Vec3::new(-1.0, -1.0, -5.0),
            glam::Vec3::new(1.0, 1.0, -3.0)
        ));
    }

    #[test]
    fn aabb_behind_camera_does_not_intersect() {
        let f = test_frustum();
        assert!(!f.intersects_aabb(
            glam::Vec3::new(-1.0, -1.0, 3.0),
            glam::Vec3::new(1.0, 1.0, 5.0)
        ));
    }

    #[test]
    fn aabb_before_near_plane_does_not_intersect() {
        let f = test_frustum();
        assert!(!f.intersects_aabb(
            glam::Vec3::new(-0.01, -0.01, -0.09),
            glam::Vec3::new(0.01, 0.01, -0.06)
        ));
    }

    #[test]
    fn aabb_beyond_far_plane_does_not_intersect() {
        let f = test_frustum();
        assert!(!f.intersects_aabb(
            glam::Vec3::new(-1.0, -1.0, -102.0),
            glam::Vec3::new(1.0, 1.0, -101.0)
        ));
    }

    #[test]
    fn aabb_outside_side_plane_does_not_intersect() {
        let f = test_frustum();
        assert!(!f.intersects_aabb(
            glam::Vec3::new(6.0, -0.5, -5.0),
            glam::Vec3::new(7.0, 0.5, -4.0)
        ));
    }

    #[test]
    fn aabb_crossing_near_plane_intersects() {
        let f = test_frustum();
        assert!(f.intersects_aabb(
            glam::Vec3::new(-0.1, -0.1, -0.2),
            glam::Vec3::new(0.1, 0.1, 0.1)
        ));
    }

    #[test]
    fn translated_rotated_camera_intersects_front_and_rejects_behind() {
        let eye = glam::Vec3::new(10.0, 2.0, 3.0);
        let forward = glam::Vec3::new(1.0, 0.0, -1.0).normalize();
        let view = glam::Mat4::look_at_rh(eye, eye + forward, glam::Vec3::Y);
        let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        let f = Frustum::from_view_proj(proj * view);

        let front_center = eye + forward * 5.0;
        assert!(f.intersects_aabb(
            front_center - glam::Vec3::splat(0.5),
            front_center + glam::Vec3::splat(0.5)
        ));

        let behind_center = eye - forward * 2.0;
        assert!(!f.intersects_aabb(
            behind_center - glam::Vec3::splat(0.5),
            behind_center + glam::Vec3::splat(0.5)
        ));
    }
}
