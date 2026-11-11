//! Climbing by pulling: grab a climbable block, and moving your hand moves you the other way.
//!
//! Vivecraft's rules, kept: a latch per hand; while latched the player is moved by the inverse of
//! the hand's motion; a latch breaks if the hand strays more than 0.5 m from where it grabbed;
//! releasing launches you with the hand's net motion over the last 0.3 s. Improvement: climbing
//! is not gated to special boots here; *any* surface the game marks climbable works, and the
//! launch speed is capped by a rule table the server owns.

use crate::{Vec3, history::History};

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

    /// `hand_room` is the hand in playspace (used for pull and launch so artificial rig motion
    /// never counts); `hand_world` the same hand in world space; `grab_point` the climbable
    /// block point under the hand, if any; `grip` whether the hand is squeezing.
    pub fn update(
        &mut self,
        rules: &ClimbRules,
        dt: f32,
        hand_room: Vec3,
        hand_world: Vec3,
        grab_point: Option<Vec3>,
        grip: bool,
    ) -> Climb {
        self.hand_history.push(dt, hand_room);
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
                let pull = -(hand_room - l.last_hand);
                l.last_hand = hand_room;
                Climb::Pull(pull)
            }
            (Some(_), false) => {
                self.latch = None;
                let v = (-self.hand_history.net_movement(LAUNCH_WINDOW) * rules.launch_scale)
                    .clamp_length_max(rules.launch_cap);
                Climb::Launch(v)
            }
            (None, false) => Climb::Free,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 90.0;

    #[test]
    fn pulling_the_hand_down_moves_the_player_up() {
        let mut c = Climber::default();
        let r = ClimbRules::default();
        let p = Vec3::new(0.0, 2.0, 0.0);
        c.update(&r, DT, Vec3::new(0.0, 1.5, 0.0), p, Some(p), true);
        let Climb::Pull(d) = c.update(&r, DT, Vec3::new(0.0, 1.4, 0.0), p, Some(p), true) else {
            panic!()
        };
        assert!((d.y - 0.1).abs() < 1e-5, "{d:?}");
    }

    #[test]
    fn no_climbable_no_latch() {
        let mut c = Climber::default();
        assert_eq!(
            c.update(
                &ClimbRules::default(),
                DT,
                Vec3::ZERO,
                Vec3::ZERO,
                None,
                true
            ),
            Climb::Free
        );
        assert!(!c.is_latched());
    }

    #[test]
    fn straying_breaks_the_latch() {
        let mut c = Climber::default();
        let r = ClimbRules::default();
        c.update(&r, DT, Vec3::ZERO, Vec3::ZERO, Some(Vec3::ZERO), true);
        assert!(c.is_latched());
        c.update(
            &r,
            DT,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 0.9),
            Some(Vec3::ZERO),
            true,
        );
        assert!(!c.is_latched());
    }

    #[test]
    fn letting_go_mid_pull_launches_and_is_capped() {
        let mut c = Climber::default();
        let r = ClimbRules::default();
        let p = Vec3::ZERO;
        c.update(&r, DT, Vec3::new(0.0, 1.5, 0.0), p, Some(p), true);
        for k in 1..=20 {
            c.update(
                &r,
                DT,
                Vec3::new(0.0, 1.5 - 0.02 * k as f32, 0.0),
                p,
                Some(p),
                true,
            );
        }
        let Climb::Launch(v) = c.update(&r, DT, Vec3::new(0.0, 1.1, 0.0), p, Some(p), false) else {
            panic!()
        };
        assert!(v.y > 0.0 && v.length() <= r.launch_cap + 1e-5, "{v:?}");
        assert!(!c.is_latched());
    }
}
