//! OpenXR types -> `qrustcraft-vr` types.

use openxr as xr;
use qrustcraft_vr::{Pose, Quat, Vec3, projection::Fov};

pub fn pose(p: xr::Posef) -> Pose {
    Pose::new(
        Vec3::new(p.position.x, p.position.y, p.position.z),
        Quat::from_xyzw(
            p.orientation.x,
            p.orientation.y,
            p.orientation.z,
            p.orientation.w,
        )
        .normalize(),
    )
}

pub fn fov(f: xr::Fovf) -> Fov {
    Fov {
        left: f.angle_left,
        right: f.angle_right,
        up: f.angle_up,
        down: f.angle_down,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_pose_round_trips() {
        let p = pose(xr::Posef::IDENTITY);
        assert_eq!(p, Pose::IDENTITY);
    }

    #[test]
    fn fov_fields_map_one_to_one() {
        let f = fov(xr::Fovf {
            angle_left: -0.9,
            angle_right: 0.8,
            angle_up: 0.7,
            angle_down: -0.6,
        });
        assert_eq!((f.left, f.right, f.up, f.down), (-0.9, 0.8, 0.7, -0.6));
    }

    #[test]
    fn unnormalised_orientation_is_fixed() {
        let p = pose(xr::Posef {
            orientation: xr::Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 2.0,
            },
            position: xr::Vector3f {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        });
        assert!((p.orientation.length() - 1.0).abs() < 1e-6);
        assert_eq!(p.position, Vec3::new(1.0, 2.0, 3.0));
    }
}
