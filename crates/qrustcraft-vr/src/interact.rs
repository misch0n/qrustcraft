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
        if dt <= 0.0 {
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
            Some(p) if dt > 0.0 => (hand_local - p).length() / dt,
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
    }
}
