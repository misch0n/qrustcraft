//! Climbing by pulling: grab a climbable block, and moving your hand moves you the other way.
//!
//! Vivecraft's rules, kept: a latch per hand; while latched the player is moved by the inverse of
//! the hand's motion; a latch breaks if the hand strays more than 0.5 m from where it grabbed;
//! releasing launches you with the hand's net motion over the last 0.3 s. Improvement: climbing
//! is not gated to special boots here; *any* surface the game marks climbable works, and the
//! launch speed is capped by a rule table the server owns.

use crate::{Pose, Quat, Vec3, history::History, rig::Rig};

pub const BREAK_DISTANCE: f32 = 0.5;
pub const LAUNCH_WINDOW: f32 = 0.3;

#[derive(Clone, Copy, Debug)]
pub struct ClimbRules {
    pub launch_scale: f32,
    pub launch_cap: f32,
}

impl Default for ClimbRules {
    fn default() -> Self {
        Self {
            launch_scale: 0.66,
            launch_cap: 0.66,
        }
    }
}

#[derive(Debug)]
struct Latch {
    point: Vec3,
    last_hand: Vec3,
}

#[derive(Debug)]
pub struct Climber {
    latch: Option<Latch>,
    hand_history: History,
}

impl Default for Climber {
    fn default() -> Self {
        Self {
            latch: None,
            hand_history: History::new(0.6),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Climb {
    /// Not climbing.
    Free,
    /// Move the player by this (world-space), and zero gravity and fall distance.
    Pull(Vec3),
    /// Let go: apply this velocity once.
    Launch(Vec3),
}

impl Climber {
    pub fn is_latched(&self) -> bool {
        self.latch.is_some()
    }

    /// `hand_room` is the hand in playspace: pull and launch are measured there, so artificial
    /// rig motion never counts, then turned into world axes and units by `rig`. `grab_point` is
    /// the climbable block point under the hand (world space), if any; `grip` whether the hand is
    /// squeezing.
    pub fn update(
        &mut self,
        rules: &ClimbRules,
        dt: f32,
        rig: &Rig,
        hand_room: Vec3,
        grab_point: Option<Vec3>,
        grip: bool,
    ) -> Climb {
        self.hand_history.push(dt, hand_room);
        let hand_world = rig.to_world(Pose::new(hand_room, Quat::IDENTITY)).position;
        match (&mut self.latch, grip) {
            (None, true) => {
                if let Some(p) = grab_point {
                    self.latch = Some(Latch {
                        point: p,
                        last_hand: hand_room,
                    });
                    return Climb::Pull(Vec3::ZERO);
                }
                Climb::Free
            }
            (Some(l), true) => {
                if hand_world.distance(l.point) > BREAK_DISTANCE {
                    self.latch = None;
                    return Climb::Launch(Vec3::ZERO);
                }
                let pull = rig.vector_to_world(l.last_hand - hand_room);
                l.last_hand = hand_room;
                Climb::Pull(pull)
            }
            (Some(_), false) => {
                self.latch = None;
                let net = rig.vector_to_world(self.hand_history.net_movement(LAUNCH_WINDOW));
                let v = (-net * rules.launch_scale).clamp_length_max(rules.launch_cap);
                Climb::Launch(v)
            }
            (None, false) => Climb::Free,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DVec3;

    const DT: f32 = 1.0 / 90.0;

    /// The world point under a room-space hand, for a grab exactly where the hand is.
    fn under(rig: &Rig, hand_room: Vec3) -> Option<Vec3> {
        Some(rig.to_world(Pose::new(hand_room, Quat::IDENTITY)).position)
    }

    #[test]
    fn pulling_the_hand_down_moves_the_player_up() {
        let (mut c, r, rig) = (Climber::default(), ClimbRules::default(), Rig::default());
        let start = Vec3::new(0.0, 1.5, 0.0);
        let p = under(&rig, start);
        c.update(&r, DT, &rig, start, p, true);
        let Climb::Pull(d) = c.update(&r, DT, &rig, Vec3::new(0.0, 1.4, 0.0), p, true) else {
            panic!()
        };
        assert!((d - Vec3::new(0.0, 0.1, 0.0)).length() < 1e-5, "{d:?}");
    }

    #[test]
    fn the_pull_is_in_world_axes_and_scale() {
        // Turned to face west (yaw 90) at double world scale, far from spawn.
        let rig = Rig {
            origin: DVec3::new(5000.0, 64.0, -800.0),
            yaw_deg: 90.0,
            world_scale: 2.0,
        };
        let (mut c, r) = (Climber::default(), ClimbRules::default());
        let start = Vec3::new(0.0, 1.5, -0.4);
        let p = under(&rig, start);
        c.update(&r, DT, &rig, start, p, true);
        // Pull the hand toward you (room +Z): you move away from it, room -Z, which is world -X.
        let Climb::Pull(d) = c.update(&r, DT, &rig, start + Vec3::new(0.0, 0.0, 0.05), p, true)
        else {
            panic!()
        };
        assert!((d - Vec3::new(-0.1, 0.0, 0.0)).length() < 1e-4, "{d:?}");
    }

    #[test]
    fn no_climbable_no_latch() {
        let mut c = Climber::default();
        let rig = Rig::default();
        assert_eq!(
            c.update(&ClimbRules::default(), DT, &rig, Vec3::ZERO, None, true),
            Climb::Free
        );
        assert!(!c.is_latched());
    }

    #[test]
    fn straying_breaks_the_latch() {
        let (mut c, r, rig) = (Climber::default(), ClimbRules::default(), Rig::default());
        c.update(&r, DT, &rig, Vec3::ZERO, Some(Vec3::ZERO), true);
        assert!(c.is_latched());
        c.update(
            &r,
            DT,
            &rig,
            Vec3::new(0.0, 0.0, 0.9),
            Some(Vec3::ZERO),
            true,
        );
        assert!(!c.is_latched());
    }

    #[test]
    fn letting_go_mid_pull_launches_and_is_capped() {
        let (mut c, r) = (Climber::default(), ClimbRules::default());
        let rig = Rig {
            yaw_deg: -135.0,
            ..Rig::default()
        };
        let start = Vec3::new(0.0, 1.5, 0.0);
        let p = under(&rig, start);
        c.update(&r, DT, &rig, start, p, true);
        for k in 1..=20 {
            c.update(
                &r,
                DT,
                &rig,
                Vec3::new(0.0, 1.5 - 0.02 * k as f32, 0.0),
                p,
                true,
            );
        }
        let Climb::Launch(v) = c.update(&r, DT, &rig, Vec3::new(0.0, 1.1, 0.0), p, false) else {
            panic!()
        };
        assert!(v.y > 0.0 && v.length() <= r.launch_cap + 1e-5, "{v:?}");
        assert!(!c.is_latched());
    }
}
