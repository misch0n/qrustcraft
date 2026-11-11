//! Asymmetric per-eye projection from OpenXR field-of-view angles.

use crate::Mat4;

/// OpenXR `XrFovf`: angles in radians from the view axis; `left` and `down` are negative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fov {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

impl Default for Fov {
    /// A plausible headset FOV (about 92 degrees each way), used for frames that aren't rendered.
    fn default() -> Self {
        Self {
            left: -0.8,
            right: 0.8,
            up: 0.8,
            down: -0.8,
        }
    }
}

impl Fov {
    /// A symmetric field of view, for simulators and tests.
    pub fn symmetric(horizontal_rad: f32, vertical_rad: f32) -> Self {
        Self {
            left: -horizontal_rad / 2.0,
            right: horizontal_rad / 2.0,
            up: vertical_rad / 2.0,
            down: -vertical_rad / 2.0,
        }
    }
}

/// Depth convention of the target renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// Near maps to 0, far maps to 1 (wgpu/Vulkan clip space, forward Z).
    ZeroToOne,
    /// Near maps to 1, far maps to 0. Horizon's world pass uses this.
    ReversedZeroToOne,
}

/// Right-handed, -Z-forward projection for an asymmetric `fov`.
pub fn projection(fov: Fov, near: f32, far: f32, depth: Depth) -> Mat4 {
    let (l, r) = (fov.left.tan(), fov.right.tan());
    let (u, d) = (fov.up.tan(), fov.down.tan());
    let (w, h) = (r - l, u - d);
    let (a, b) = match depth {
        Depth::ZeroToOne => (far / (near - far), far * near / (near - far)),
        Depth::ReversedZeroToOne => (near / (far - near), far * near / (far - near)),
    };
    Mat4::from_cols_array(&[
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / h,
        0.0,
        0.0,
        (r + l) / w,
        (u + d) / h,
        a,
        -1.0,
        0.0,
        0.0,
        b,
        0.0,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec4;

    fn ndc(m: Mat4, p: Vec4) -> glam::Vec3 {
        let c = m * p;
        glam::Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w)
    }

    #[test]
    fn symmetric_matches_glam_perspective() {
        let fov = Fov::symmetric(1.2, 1.0);
        let ours = projection(fov, 0.05, 100.0, Depth::ZeroToOne);
        let aspect = (1.2f32 / 2.0).tan() / (0.5f32).tan();
        let theirs = Mat4::perspective_rh(1.0, aspect, 0.05, 100.0);
        assert!(ours.abs_diff_eq(theirs, 1e-5), "{ours:?}\n{theirs:?}");
    }

    #[test]
    fn reversed_depth_swaps_planes() {
        let fov = Fov::symmetric(1.5, 1.5);
        let m = projection(fov, 0.05, 512.0, Depth::ReversedZeroToOne);
        assert!((ndc(m, Vec4::new(0.0, 0.0, -0.05, 1.0)).z - 1.0).abs() < 1e-4);
        assert!(ndc(m, Vec4::new(0.0, 0.0, -512.0, 1.0)).z.abs() < 1e-4);
    }

    #[test]
    fn asymmetric_fov_edges_land_on_ndc_edges() {
        // Quest-like: wider outward than inward, shifted look.
        let fov = Fov {
            left: -0.9,
            right: 0.7,
            up: 0.8,
            down: -0.85,
        };
        let m = projection(fov, 0.1, 100.0, Depth::ZeroToOne);
        let z = -10.0;
        let edge = |ang: f32| ang.tan() * -z;
        assert!((ndc(m, Vec4::new(edge(fov.left), 0.0, z, 1.0)).x + 1.0).abs() < 1e-4);
        assert!((ndc(m, Vec4::new(edge(fov.right), 0.0, z, 1.0)).x - 1.0).abs() < 1e-4);
        assert!((ndc(m, Vec4::new(0.0, edge(fov.up), z, 1.0)).y - 1.0).abs() < 1e-4);
        assert!((ndc(m, Vec4::new(0.0, edge(fov.down), z, 1.0)).y + 1.0).abs() < 1e-4);
    }
}
