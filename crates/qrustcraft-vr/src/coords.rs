//! OpenXR <-> Minecraft coordinate conversion.
//!
//! OpenXR: +X right, +Y up, -Z forward. Minecraft at yaw 0 looks toward +Z, so the playspace maps
//! to the world by a half turn about Y, then the rig's yaw. Minecraft yaw grows clockwise seen from
//! above, i.e. it is a rotation about +Y by `-yaw`.

use crate::{Pose, Quat, Vec3};

/// Rotation applied to a vector in the rig's own frame to express it in game axes, for a rig that
/// faces game yaw `yaw_deg`.
pub fn rig_rotation(yaw_deg: f32) -> Quat {
    Quat::from_rotation_y(-yaw_deg.to_radians()) * Quat::from_rotation_y(std::f32::consts::PI)
}

/// Minecraft yaw (degrees, 0 = +Z, 90 = -X) of a forward vector. Zero for a vertical vector.
pub fn yaw_of(forward: Vec3) -> f32 {
    if forward.x.abs() < 1e-6 && forward.z.abs() < 1e-6 {
        return 0.0;
    }
    (-forward.x).atan2(forward.z).to_degrees()
}

/// Minecraft pitch (degrees, positive looking down) of a forward vector.
pub fn pitch_of(forward: Vec3) -> f32 {
    (-forward.y).clamp(-1.0, 1.0).asin().to_degrees()
}

/// Transform a pose in the rig's playspace into the rig-oriented game frame (no translation of the
/// rig origin: the caller adds it).
pub fn pose_to_game(local: Pose, yaw_deg: f32) -> Pose {
    let r = rig_rotation(yaw_deg);
    Pose {
        position: r * local.position,
        orientation: r * local.orientation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn xr_forward_is_minecraft_south_at_yaw_zero() {
        let f = rig_rotation(0.0) * Vec3::NEG_Z;
        assert!(close(f, Vec3::Z));
        assert!(yaw_of(f).abs() < 1e-4);
    }

    #[test]
    fn xr_right_is_west_at_yaw_zero() {
        // Facing south, your right hand points west (-X).
        assert!(close(rig_rotation(0.0) * Vec3::X, Vec3::NEG_X));
    }

    #[test]
    fn yaw_90_faces_west() {
        let f = rig_rotation(90.0) * Vec3::NEG_Z;
        assert!(close(f, Vec3::NEG_X), "{f:?}");
        assert!((yaw_of(f) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn yaw_round_trips() {
        for y in [-170.0f32, -90.0, -10.0, 0.0, 45.0, 135.0, 179.0] {
            let f = rig_rotation(y) * Vec3::NEG_Z;
            assert!((yaw_of(f) - y).abs() < 1e-3, "{y}");
        }
    }

    #[test]
    fn pitch_positive_looks_down() {
        assert!((pitch_of(Vec3::new(0.0, -1.0, 0.0)) - 90.0).abs() < 1e-4);
        assert!(pitch_of(Vec3::Z).abs() < 1e-4);
    }
}
