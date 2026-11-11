//! The room-scale rig: where the player's playspace sits in the world.
//!
//! Vivecraft's approach, kept: the physical room is a rectangle floating in the world at
//! `origin` facing `yaw_deg`. Walking in the room moves the head inside it; the game then drags
//! the *player entity* toward the head (collision applies), and the rig slides so the head stays
//! over the entity ([`Rig::absorb_walk`]). Turning pivots around the head, never around the room
//! origin, so a snap turn cannot teleport you sideways.

use crate::{
    DVec3, EyeView, Frame, Hand, Pose, Vec3, coords,
    projection::{Depth, Fov, projection},
};

#[derive(Clone, Copy, Debug)]
pub struct Rig {
    /// World position of the playspace origin (the floor point under the room's centre). `f64`,
    /// like the game's own positions: an `f32` here moves the head in visible steps far from
    /// spawn (about 8 mm at 100 000 blocks).
    pub origin: DVec3,
    /// Game yaw of the playspace's forward, degrees.
    pub yaw_deg: f32,
    /// World scale: 1.0 is a normal Steve (1.8 m eye height is about 1.62 blocks).
    pub world_scale: f32,
}

impl Default for Rig {
    fn default() -> Self {
        Self {
            origin: DVec3::ZERO,
            yaw_deg: 0.0,
            world_scale: 1.0,
        }
    }
}

/// Per-eye input for [`Rig::frame`].
#[derive(Clone, Copy, Debug)]
pub struct EyeInput {
    /// Eye pose in playspace.
    pub local: Pose,
    pub fov: Fov,
}

impl Rig {
    /// A playspace displacement (no origin) in game axes and world units.
    pub fn vector_to_world(&self, local: Vec3) -> Vec3 {
        coords::rig_rotation(self.yaw_deg) * local * self.world_scale
    }

    /// Playspace pose -> game-space pose relative to the world point `anchor`. The large part
    /// (`origin - anchor`) is taken in `f64`, so the result is exact however far from spawn.
    pub fn to_relative(&self, local: Pose, anchor: DVec3) -> Pose {
        let g = coords::pose_to_game(local, self.yaw_deg);
        Pose {
            position: (self.origin - anchor).as_vec3() + g.position * self.world_scale,
            orientation: g.orientation,
        }
    }

    /// Playspace pose -> world pose (world position, game orientation). The position is `f32`:
    /// good to a few millimetres anywhere a player goes, which is fine for gameplay (aim, reach,
    /// climbing), but render from [`Rig::frame`], which is camera-relative.
    pub fn to_world(&self, local: Pose) -> Pose {
        self.to_relative(local, DVec3::ZERO)
    }

    /// World position of a playspace point, at full precision.
    pub fn point_to_world(&self, local: Vec3) -> DVec3 {
        self.origin + self.vector_to_world(local).as_dvec3()
    }

    /// Build the frame the renderer needs. `camera_origin` is the world point the renderer treats
    /// as zero (Horizon is camera-relative); every pose in the frame is relative to it.
    pub fn frame(
        &self,
        head: Pose,
        eyes: [EyeInput; 2],
        hands: [Option<Pose>; 2],
        camera_origin: DVec3,
        near: f32,
        far: f32,
        depth: Depth,
    ) -> Frame {
        let make = |e: EyeInput| {
            let pose = self.to_relative(e.local, camera_origin);
            EyeView {
                pose,
                view: pose.to_mat4().inverse(),
                projection: projection(e.fov, near, far, depth),
            }
        };
        let rel = |p: Option<Pose>| p.map(|p| self.to_relative(p, camera_origin));
        Frame {
            eyes: [make(eyes[0]), make(eyes[1])],
            head: self.to_relative(head, camera_origin),
            hands: [rel(hands[0]), rel(hands[1])],
        }
    }

    /// The player entity moved by `entity_delta` (after collision) while the head walked from
    /// `head_local_old` to `head_local_new` in playspace. Slide the rig so the head ends up over
    /// the entity again: walking into a wall pushes the room back rather than letting the head
    /// through, and the room rides up and down with the body (stairs, jumps, falls). The head's
    /// own vertical motion is posture (crouching, standing up), not walking, and never drags the
    /// body.
    ///
    /// Returns the part of this step's walk the entity could not follow (world space,
    /// horizontal): zero in the open, non-zero against a wall. Callers fade the view with it (see
    /// `comfort`).
    pub fn absorb_walk(
        &mut self,
        head_local_old: Vec3,
        head_local_new: Vec3,
        entity_delta: Vec3,
    ) -> Vec3 {
        let mut wanted = self.vector_to_world(head_local_new - head_local_old);
        wanted.y = 0.0;
        // Moving the rig by (entity_delta - wanted) puts the head at old_world + entity_delta.
        self.origin += (entity_delta - wanted).as_dvec3();
        let mut residual = wanted - entity_delta;
        residual.y = 0.0;
        residual
    }

    /// Turn the rig by `delta_deg` (positive = clockwise from above, like Minecraft yaw) about the
    /// head's current *local* position.
    pub fn turn_about_head(&mut self, head_local: Vec3, delta_deg: f32) {
        let pivot_before = self.vector_to_world(head_local);
        self.yaw_deg = wrap_deg(self.yaw_deg + delta_deg);
        let pivot_after = self.vector_to_world(head_local);
        self.origin += (pivot_before - pivot_after).as_dvec3();
    }

    /// Put the head's horizontal position over world point `p`, with the floor at `p.y` (used by
    /// teleport and respawn).
    pub fn place_head_over(&mut self, head_local: Vec3, p: DVec3) {
        let h = self.vector_to_world(head_local).as_dvec3();
        self.origin = DVec3::new(p.x - h.x, p.y, p.z - h.z);
    }

    /// Re-centre: face `yaw_deg` and put the head over the world point `head_world_xz`.
    pub fn recenter(&mut self, head_local: Vec3, head_world_xz: (f64, f64), yaw_deg: f32) {
        self.yaw_deg = wrap_deg(yaw_deg);
        self.place_head_over(
            head_local,
            DVec3::new(head_world_xz.0, self.origin.y, head_world_xz.1),
        );
    }

    /// World-space hand pose for the interaction code.
    pub fn hand_world(&self, hand_local: Pose) -> Pose {
        self.to_world(hand_local)
    }

    /// Which hand is "dominant" is a setting; this keeps the mapping in one place.
    pub fn main_hand(right_handed: bool) -> Hand {
        if right_handed {
            Hand::Right
        } else {
            Hand::Left
        }
    }
}

pub fn wrap_deg(d: f32) -> f32 {
    let mut x = (d + 180.0).rem_euclid(360.0) - 180.0;
    if x == -180.0 {
        x = 180.0;
    }
    x
}

/// Face a head orientation's yaw/pitch for the game's `y_rot`/`x_rot`.
pub fn head_yaw_pitch(head_world: &Pose) -> (f32, f32) {
    let f = head_world.forward();
    (coords::yaw_of(f), coords::pitch_of(f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quat;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn world_position_adds_origin() {
        let rig = Rig {
            origin: DVec3::new(10.0, 64.0, -5.0),
            ..Rig::default()
        };
        let w = rig.to_world(Pose::new(Vec3::new(0.0, 1.7, 0.0), Quat::IDENTITY));
        assert!(close(w.position, Vec3::new(10.0, 65.7, -5.0)));
    }

    #[test]
    fn world_scale_scales_room_not_orientation() {
        let rig = Rig {
            world_scale: 2.0,
            ..Rig::default()
        };
        let w = rig.to_world(Pose::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY));
        assert!(close(w.position, Vec3::new(0.0, 2.0, 0.0)));
    }

    #[test]
    fn snap_turn_keeps_head_in_place() {
        let mut rig = Rig {
            origin: DVec3::new(3.0, 70.0, 4.0),
            ..Rig::default()
        };
        let head = Vec3::new(0.4, 1.7, -0.3);
        let before = rig.to_world(Pose::new(head, Quat::IDENTITY)).position;
        rig.turn_about_head(head, 45.0);
        let after = rig.to_world(Pose::new(head, Quat::IDENTITY)).position;
        assert!(close(before, after), "{before:?} vs {after:?}");
        assert!((rig.yaw_deg - 45.0).abs() < 1e-4);
    }

    #[test]
    fn turn_changes_where_you_face() {
        let mut rig = Rig::default();
        rig.turn_about_head(Vec3::ZERO, 90.0);
        let f = rig.to_world(Pose::IDENTITY).forward();
        assert!(close(f, Vec3::NEG_X), "{f:?}");
    }

    #[test]
    fn walking_with_free_entity_leaves_no_residual() {
        let mut rig = Rig::default();
        let old = Vec3::new(0.0, 1.7, 0.0);
        let new = Vec3::new(0.5, 1.7, -0.5); // right and forward in the room
        let moved = coords::rig_rotation(0.0) * (new - old);
        let head_world_before = rig.to_world(Pose::new(old, Quat::IDENTITY)).position;
        let residual = rig.absorb_walk(old, new, moved);
        assert!(residual.length() < 1e-5);
        // The entity moved by `moved`; the head is now exactly over it.
        let head_world_after = rig.to_world(Pose::new(new, Quat::IDENTITY)).position;
        assert!(close(head_world_after, head_world_before + moved));
    }

    #[test]
    fn walking_into_a_wall_reports_residual() {
        let mut rig = Rig::default();
        let old = Vec3::new(0.0, 1.7, 0.0);
        let new = Vec3::new(0.0, 1.7, -1.0); // one metre forward = +Z in the world
        let residual = rig.absorb_walk(old, new, Vec3::ZERO); // entity blocked
        assert!(close(residual, Vec3::new(0.0, 0.0, 1.0)), "{residual:?}");
    }

    #[test]
    fn the_room_rides_with_the_body_but_crouching_does_not_move_it() {
        let mut rig = Rig::default();
        // Stepped up a stair while standing still in the room.
        let head = Vec3::new(0.0, 1.7, 0.0);
        rig.absorb_walk(head, head, Vec3::new(0.0, 0.5, 0.0));
        assert!((rig.origin.y - 0.5).abs() < 1e-6);
        // Crouched: the head dropped, the body did not move.
        let r = rig.absorb_walk(head, Vec3::new(0.0, 1.2, 0.0), Vec3::ZERO);
        assert!((rig.origin.y - 0.5).abs() < 1e-6 && r.length() < 1e-6);
    }

    #[test]
    fn walking_follows_the_rig_yaw() {
        let mut rig = Rig {
            yaw_deg: 90.0,
            ..Rig::default()
        };
        let old = Vec3::new(0.0, 1.7, 0.0);
        let new = Vec3::new(0.0, 1.7, -1.0); // one metre forward in the room = -X at yaw 90
        let residual = rig.absorb_walk(old, new, Vec3::ZERO);
        assert!(close(residual, Vec3::new(-1.0, 0.0, 0.0)), "{residual:?}");
    }

    #[test]
    fn far_from_spawn_the_eyes_stay_exact() {
        let far = DVec3::new(1.0e5, 64.0, -3.0e5);
        let rig = Rig {
            origin: far,
            ..Rig::default()
        };
        let eye = EyeInput {
            local: Pose::new(Vec3::new(0.0321, 1.6, 0.0), Quat::IDENTITY),
            fov: Fov::symmetric(1.5, 1.5),
        };
        let cam = far + DVec3::new(0.0, 1.6, 0.0);
        let f = rig.frame(
            eye.local,
            [eye, eye],
            [None, None],
            cam,
            0.05,
            512.0,
            Depth::ReversedZeroToOne,
        );
        // At yaw 0 room +X is world -X. An f32 world position here is only good to ~3 cm.
        assert!(close(f.eyes[0].pose.position, Vec3::new(-0.0321, 0.0, 0.0)));
        assert!((rig.point_to_world(eye.local.position).x - (1.0e5 - 0.0321)).abs() < 1e-6);
    }

    #[test]
    fn placing_the_head_over_a_point() {
        let mut rig = Rig {
            yaw_deg: 30.0,
            ..Rig::default()
        };
        let head = Vec3::new(0.4, 1.7, -0.2);
        let target = DVec3::new(2.0e5 + 0.25, 70.0, 5.5);
        rig.place_head_over(head, target);
        let h = rig.point_to_world(head);
        assert!((h.x - target.x).abs() < 1e-6 && (h.z - target.z).abs() < 1e-6);
        assert_eq!(rig.origin.y, 70.0);
    }

    #[test]
    fn wrap() {
        assert_eq!(wrap_deg(190.0), -170.0);
        assert_eq!(wrap_deg(-190.0), 170.0);
        assert_eq!(wrap_deg(180.0), 180.0);
    }

    #[test]
    fn frame_is_camera_relative() {
        let rig = Rig {
            origin: DVec3::new(100.0, 64.0, 100.0),
            ..Rig::default()
        };
        let eye = EyeInput {
            local: Pose::new(Vec3::new(0.0, 1.6, 0.0), Quat::IDENTITY),
            fov: Fov::symmetric(1.5, 1.5),
        };
        let cam = DVec3::new(100.0, 65.6, 100.0);
        let f = rig.frame(
            eye.local,
            [eye, eye],
            [None, None],
            cam,
            0.05,
            512.0,
            Depth::ReversedZeroToOne,
        );
        assert!(close(f.eyes[0].pose.position, Vec3::ZERO));
        assert!(
            f.eyes[0]
                .view
                .abs_diff_eq(f.eyes[0].pose.to_mat4().inverse(), 1e-6)
        );
    }
}
