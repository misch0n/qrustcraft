//! Hand interaction: aim rays and swing-to-mine/attack.
//!
//! Vivecraft's best idea, kept: tools act where the *hand* points, not where the head looks, and
//! hitting requires physically swinging. We detect a swing as hand speed along the tool's axis
//! crossing a threshold, with a refractory period so one swing is one hit.

use crate::{Pose, Vec3};

/// Survival reach in blocks (vanilla `block_interaction_range`); creative gets 5.
pub const REACH_SURVIVAL: f32 = 4.5;
pub const REACH_CREATIVE: f32 = 5.0;

#[derive(Clone, Copy, Debug)]
pub struct AimRay {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// Ray from a hand. `pointing_offset_deg` tilts the ray down from the controller's -Z axis,
/// because a Touch controller held naturally points about 35 degrees below its grip axis.
pub fn aim_ray(hand: &Pose, pointing_offset_deg: f32) -> AimRay {
    let tilt = crate::Quat::from_rotation_x(-pointing_offset_deg.to_radians());
    AimRay {
        origin: hand.position,
        direction: (hand.orientation * tilt * Vec3::NEG_Z).normalize(),
    }
}

/// No real hand moves faster than this (m/s; a pitcher's hand peaks around 20). A faster step
/// between two samples is the tracker re-acquiring a controller, not motion, and must not count.
pub const MAX_HAND_SPEED: f32 = 25.0;

fn is_tracking_jump(from: Vec3, to: Vec3, dt: f32) -> bool {
    dt > 0.0 && (to - from).length() / dt > MAX_HAND_SPEED
}

#[derive(Clone, Copy, Debug)]
pub struct SwingConfig {
    /// Hand speed (m/s) along the tool axis that counts as a swing.
    pub speed_threshold: f32,
    /// Seconds before another swing may register.
    pub cooldown: f32,
}

impl Default for SwingConfig {
    fn default() -> Self {
        Self {
            speed_threshold: 1.6,
            cooldown: 0.25,
        }
    }
}

/// Tracks hand motion; reports one `true` per swing.
#[derive(Debug, Default)]
pub struct SwingDetector {
    last: Option<Vec3>,
    cooldown_left: f32,
}

impl SwingDetector {
    /// `hand_world` is the hand position in world space (metres); `tool_axis` the unit direction
    /// the tool points. A swing is motion *toward* where the tool points (a stab/chop), measured
    /// from rig-independent hand velocity so that artificial rig motion never counts.
    pub fn update(
        &mut self,
        cfg: &SwingConfig,
        dt: f32,
        hand_local: Vec3,
        tool_axis: Vec3,
    ) -> bool {
        self.cooldown_left = (self.cooldown_left - dt).max(0.0);
        let Some(prev) = self.last.replace(hand_local) else {
            return false;
        };
        if dt <= 0.0 || is_tracking_jump(prev, hand_local, dt) {
            return false;
        }
        let along = ((hand_local - prev) / dt).dot(tool_axis);
        if along > cfg.speed_threshold && self.cooldown_left == 0.0 {
            self.cooldown_left = cfg.cooldown;
            return true;
        }
        false
    }
}

/// Sustained mining: true while the tool is touching a block and the hand keeps *moving* (a
/// scrubbing/pecking motion), so holding the arm still on a block does nothing, as in Vivecraft.
#[derive(Debug, Default)]
pub struct MiningHold {
    last: Option<Vec3>,
    energy: f32,
}

impl MiningHold {
    pub fn update(&mut self, dt: f32, hand_local: Vec3, touching_block: bool) -> bool {
        let speed = match self.last.replace(hand_local) {
            Some(p) if dt > 0.0 && !is_tracking_jump(p, hand_local, dt) => {
                (hand_local - p).length() / dt
            }
            _ => 0.0,
        };
        // Leaky integrator of hand speed: sustained motion keeps it up, one jerk decays.
        self.energy = (self.energy + speed * dt * 2.0 - dt * 0.8).clamp(0.0, 1.0);
        touching_block && self.energy > 0.15
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quat;

    #[test]
    fn aim_ray_tilts_down() {
        let r = aim_ray(&Pose::new(Vec3::ZERO, Quat::IDENTITY), 35.0);
        assert!(r.direction.y < 0.0 && r.direction.z < 0.0);
        assert!((r.direction.length() - 1.0).abs() < 1e-5);
        let flat = aim_ray(&Pose::IDENTITY, 0.0);
        assert!((flat.direction - Vec3::NEG_Z).length() < 1e-5);
    }

    #[test]
    fn a_stab_swings_once_with_cooldown() {
        let cfg = SwingConfig::default();
        let mut d = SwingDetector::default();
        let axis = Vec3::NEG_Z;
        assert!(!d.update(&cfg, 0.011, Vec3::ZERO, axis));
        // 3 m/s forward.
        assert!(d.update(&cfg, 0.011, Vec3::new(0.0, 0.0, -0.033), axis));
        // still moving fast but inside cooldown
        assert!(!d.update(&cfg, 0.011, Vec3::new(0.0, 0.0, -0.066), axis));
    }

    #[test]
    fn retreating_or_sideways_is_not_a_swing() {
        let cfg = SwingConfig::default();
        let mut d = SwingDetector::default();
        d.update(&cfg, 0.011, Vec3::ZERO, Vec3::NEG_Z);
        assert!(!d.update(&cfg, 0.011, Vec3::new(0.05, 0.0, 0.0), Vec3::NEG_Z));
        assert!(!d.update(&cfg, 0.011, Vec3::new(0.05, 0.0, 0.05), Vec3::NEG_Z));
    }

    #[test]
    fn mining_needs_motion_and_contact() {
        let mut m = MiningHold::default();
        let mut p = Vec3::ZERO;
        let mut on = false;
        for k in 0..30 {
            p.x = if k % 2 == 0 { 0.0 } else { 0.02 };
            on = m.update(0.011, p, true);
        }
        assert!(on, "scrubbing should mine");
        let mut m = MiningHold::default();
        for _ in 0..60 {
            assert!(
                !m.update(0.011, Vec3::ZERO, true),
                "a still hand must not mine"
            );
        }
        let mut m = MiningHold::default();
        for k in 0..30 {
            assert!(!m.update(0.011, Vec3::new(k as f32 * 0.02, 0.0, 0.0), false));
        }
        // A tracking jump is not motion.
        let mut m = MiningHold::default();
        m.update(0.011, Vec3::ZERO, true);
        assert!(!m.update(0.011, Vec3::new(0.5, 0.0, 0.0), true));
    }

    #[test]
    fn a_tracking_jump_is_not_a_stab() {
        let cfg = SwingConfig::default();
        let mut d = SwingDetector::default();
        d.update(&cfg, 0.011, Vec3::ZERO, Vec3::NEG_Z);
        assert!(!d.update(&cfg, 0.011, Vec3::new(0.0, 0.0, -0.5), Vec3::NEG_Z));
    }
}

// ---------------------------------------------------------------------------------------------
// Windowed swing tracking and per-class reach (Vivecraft's SwingTracker, on fixed-step history).

use crate::history::History;

/// What the hand holds, for how far its hits reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolClass {
    /// Swords, lances, anything weapon-shaped.
    Weapon,
    /// Pickaxes, axes, shovels, hoes.
    Tool,
    /// Everything else a hand can hold.
    Other,
    Empty,
}

impl ToolClass {
    /// Distance from the hand a hit can land, metres, for an arm of `arm_length` (Vivecraft's
    /// measured defaults: about 2.5, 1.55, 0.4, 0.3 for a 0.7 m arm). Scaling by the real arm
    /// replaces its fixed constants, so a child's sword is shorter-reaching and the world feels
    /// proportionate.
    pub fn reach(self, arm_length: f32) -> f32 {
        let base = match self {
            ToolClass::Weapon => 2.5,
            ToolClass::Tool => 1.55,
            ToolClass::Other => 0.4,
            ToolClass::Empty => 0.3,
        };
        base * (arm_length / 0.7)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SwingTrackConfig {
    /// Tip speed (m/s) averaged over `window` that counts as a swing (Vivecraft: 2.5).
    pub speed: f32,
    pub window: f32,
    /// How far ahead of the hand the tracked tip sits (Vivecraft: 0.3 m).
    pub tip_offset: f32,
    pub cooldown: f32,
    /// Creative mode needs a harder swing (x1.5), so block-breaking isn't accidental.
    pub creative_factor: f32,
}

impl Default for SwingTrackConfig {
    fn default() -> Self {
        Self {
            speed: 2.5,
            window: 0.33,
            tip_offset: 0.3,
            cooldown: 0.25,
            creative_factor: 1.5,
        }
    }
}

/// One hand's swing tracker. Feed room-space (not world-space) hand poses so rig motion never
/// counts as a swing.
#[derive(Debug)]
pub struct SwingTracker {
    tip: History,
    cooldown_left: f32,
}

impl Default for SwingTracker {
    fn default() -> Self {
        Self {
            tip: History::new(0.6),
            cooldown_left: 0.0,
        }
    }
}

/// A registered swing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swing {
    pub speed: f32,
    /// Extra hold-ticks worth of mining damage: 3 + min(speed - threshold, 4), like Vivecraft,
    /// but returned as a number for the game to apply instead of faking repeated clicks.
    pub mining_hits: f32,
}

impl SwingTracker {
    /// `hand_room` is `None` while the hand is not tracked: the history is dropped, so the jump
    /// to wherever the controller is re-acquired never reads as a swing.
    pub fn update(
        &mut self,
        cfg: &SwingTrackConfig,
        dt: f32,
        hand_room: Option<&Pose>,
        creative: bool,
    ) -> Option<Swing> {
        self.cooldown_left = (self.cooldown_left - dt).max(0.0);
        let Some(hand_room) = hand_room else {
            self.tip.clear();
            return None;
        };
        let tip = hand_room.position + hand_room.forward() * cfg.tip_offset;
        if self
            .tip
            .latest()
            .is_some_and(|prev| is_tracking_jump(prev, tip, dt))
        {
            // Tracked again, but the pose leapt: start over from here.
            self.tip.clear();
        }
        self.tip.push(dt, tip);
        let threshold = cfg.speed * if creative { cfg.creative_factor } else { 1.0 };
        let speed = self.tip.average_speed(cfg.window);
        if speed > threshold && self.cooldown_left == 0.0 {
            self.cooldown_left = cfg.cooldown;
            return Some(Swing {
                speed,
                mining_hits: 3.0 + (speed - threshold).min(4.0),
            });
        }
        None
    }
}

#[cfg(test)]
mod swing_tests {
    use super::*;
    use crate::Quat;

    fn swing_at(speed: f32, hz: f32, creative: bool) -> Option<Swing> {
        let (cfg, mut t) = (SwingTrackConfig::default(), SwingTracker::default());
        let dt = 1.0 / hz;
        let mut hit = None;
        for k in 0..(0.5 * hz) as usize {
            let p = Pose::new(Vec3::new(speed * k as f32 * dt, 0.0, 0.0), Quat::IDENTITY);
            hit = hit.or(t.update(&cfg, dt, Some(&p), creative));
        }
        hit
    }

    #[test]
    fn a_fast_swing_registers_at_any_refresh_rate() {
        for hz in [72.0, 90.0, 120.0] {
            assert!(swing_at(4.0, hz, false).is_some(), "{hz} Hz");
        }
    }

    #[test]
    fn regaining_tracking_elsewhere_is_not_a_swing() {
        let (cfg, mut t) = (SwingTrackConfig::default(), SwingTracker::default());
        let dt = 1.0 / 90.0;
        let still = Pose::new(Vec3::ZERO, Quat::IDENTITY);
        let moved = Pose::new(Vec3::new(0.0, 0.4, -0.3), Quat::IDENTITY);
        for _ in 0..10 {
            assert!(t.update(&cfg, dt, Some(&still), false).is_none());
        }
        // Lost for a few frames, then back half a metre away.
        for _ in 0..3 {
            assert!(t.update(&cfg, dt, None, false).is_none());
        }
        for _ in 0..30 {
            assert!(t.update(&cfg, dt, Some(&moved), false).is_none());
        }
        // Same leap with no gap in tracking (the runtime snapped the pose): also not a swing.
        let mut t = SwingTracker::default();
        for _ in 0..10 {
            t.update(&cfg, dt, Some(&still), false);
        }
        for _ in 0..30 {
            assert!(t.update(&cfg, dt, Some(&moved), false).is_none());
        }
    }

    #[test]
    fn slow_waving_does_not() {
        assert!(swing_at(1.0, 90.0, false).is_none());
    }

    #[test]
    fn creative_needs_a_harder_swing() {
        assert!(swing_at(3.0, 90.0, false).is_some());
        assert!(swing_at(3.0, 90.0, true).is_none());
        assert!(swing_at(4.5, 90.0, true).is_some());
    }

    #[test]
    fn harder_swings_mine_more_up_to_a_cap() {
        let soft = swing_at(3.0, 90.0, false).unwrap().mining_hits;
        let hard = swing_at(20.0, 90.0, false).unwrap().mining_hits;
        assert!(hard > soft && hard <= 7.0);
    }

    #[test]
    fn reach_scales_with_arm_and_class() {
        assert!(ToolClass::Weapon.reach(0.7) > ToolClass::Tool.reach(0.7));
        assert!(ToolClass::Tool.reach(0.7) > ToolClass::Other.reach(0.7));
        assert!(ToolClass::Weapon.reach(0.5) < ToolClass::Weapon.reach(0.8));
        assert!((ToolClass::Weapon.reach(0.7) - 2.5).abs() < 1e-6);
    }
}
