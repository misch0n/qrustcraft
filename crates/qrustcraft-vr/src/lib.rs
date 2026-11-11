//! Engine-agnostic VR core for a Rust Minecraft client.
//!
//! No GPU, OS or OpenXR dependency: everything here is plain math and state machines, so it is
//! unit-tested on any machine and can be driven by OpenXR (`qrustcraft-xr`), a simulator, or a
//! replay file. The game side only ever sees [`Frame`]: per-eye view/projection matrices, the
//! hands' poses, and a [`input::Intent`] to apply.
//!
//! Coordinates: the game world is Minecraft's (+X east, +Y up, +Z south, yaw 0 faces +Z, yaw grows
//! clockwise seen from above). OpenXR is +X right, +Y up, -Z forward. See [`coords`].

pub mod bow;
pub mod climb;
pub mod comfort;
pub mod coords;
pub mod haptics;
pub mod history;
pub mod input;
pub mod interact;
pub mod pose_net;
pub mod posture;
pub mod projection;
pub mod rig;
pub mod teleport;
pub mod ui;

pub use glam::{Mat4, Quat, Vec2, Vec3};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::{Depth, Fov, projection};

    fn eye(fov: Fov) -> EyeView {
        EyeView {
            pose: Pose::IDENTITY,
            view: Mat4::IDENTITY,
            projection: projection(fov, 0.05, 512.0, Depth::ReversedZeroToOne),
        }
    }

    #[test]
    fn symmetric_eye_reports_no_asymmetry() {
        let e = eye(Fov::symmetric(1.6, 1.4));
        assert!(e.asymmetry().length() < 1e-6);
        assert!((e.tan_half_fov_y() - 0.7f32.tan()).abs() < 1e-5);
        assert!((e.aspect() - 0.8f32.tan() / 0.7f32.tan()).abs() < 1e-5);
    }

    #[test]
    fn asymmetric_eye_is_off_axis() {
        // Wider to the left than the right: the lens axis sits right of NDC centre.
        let e = eye(Fov {
            left: -1.0,
            right: 0.6,
            up: 0.8,
            down: -0.8,
        });
        assert!(e.asymmetry().x.abs() > 0.1);
        assert!(e.asymmetry().y.abs() < 1e-6);
    }
}

/// A rigid transform: position in metres and orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub orientation: Quat,
}

impl Pose {
    pub const IDENTITY: Pose = Pose {
        position: Vec3::ZERO,
        orientation: Quat::IDENTITY,
    };

    pub fn new(position: Vec3, orientation: Quat) -> Self {
        Self {
            position,
            orientation,
        }
    }

    pub fn forward(&self) -> Vec3 {
        self.orientation * Vec3::NEG_Z
    }

    pub fn up(&self) -> Vec3 {
        self.orientation * Vec3::Y
    }

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.orientation, self.position)
    }
}

/// Which hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hand {
    Left,
    Right,
}

impl Hand {
    pub fn other(self) -> Hand {
        match self {
            Hand::Left => Hand::Right,
            Hand::Right => Hand::Left,
        }
    }
}

/// One eye's rendering parameters, already in game space.
#[derive(Clone, Copy, Debug)]
pub struct EyeView {
    /// Eye pose in game space, relative to the origin the caller passed to the rig.
    pub pose: Pose,
    /// World-to-eye matrix (camera-relative when the rig origin is the camera).
    pub view: Mat4,
    pub projection: Mat4,
}

impl EyeView {
    /// Width over height of the eye's image, from the projection (Horizon's `View::aspect`).
    pub fn aspect(&self) -> f32 {
        self.projection.y_axis.y / self.projection.x_axis.x
    }

    /// `tan(fov_y / 2)` for a symmetric lens. A headset's lens is asymmetric: shaders that rebuild
    /// view rays from this and the aspect alone are off-centre by `asymmetry()`. See
    /// `docs/INTEGRATION.md` ("asymmetric FOV").
    pub fn tan_half_fov_y(&self) -> f32 {
        1.0 / self.projection.y_axis.y
    }

    /// Horizontal and vertical NDC offset of the lens axis (0, 0 for a symmetric lens). Row 0/1 of
    /// the projection's z column holds `(r + l) / (r - l)` and `(u + d) / (u - d)`.
    pub fn asymmetry(&self) -> Vec2 {
        Vec2::new(self.projection.z_axis.x, self.projection.z_axis.y)
    }
}

/// Everything the game needs for one rendered frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub eyes: [EyeView; 2],
    /// Head pose in game space.
    pub head: Pose,
    /// Hand poses in game space, when tracked.
    pub hands: [Option<Pose>; 2],
}
