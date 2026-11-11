//! Comfort: a vignette that grows with artificial (non-head) motion.
//!
//! Head motion is the player's own and never vignettes; only motion the game imposes does
//! (smooth turning, smooth locomotion, being pushed, riding).

#[derive(Clone, Copy, Debug)]
pub struct ComfortSettings {
    /// Vignette strength at full speed, 0..1.
    pub strength: f32,
    /// Angular speed (deg/s) at which the vignette reaches `strength`.
    pub full_turn_dps: f32,
    /// Linear speed (m/s) at which the vignette reaches `strength`.
    pub full_move_mps: f32,
    /// How fast the vignette relaxes, per second.
    pub release_rate: f32,
}

impl Default for ComfortSettings {
    fn default() -> Self {
        Self {
            strength: 0.6,
            full_turn_dps: 120.0,
            full_move_mps: 6.0,
            release_rate: 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vignette {
    level: f32,
}

impl Vignette {
    /// Advance by `dt` seconds with the artificial `turn_dps` and `move_mps` this frame.
    /// Returns the current level in 0..=1 for the shader.
    pub fn update(&mut self, s: &ComfortSettings, dt: f32, turn_dps: f32, move_mps: f32) -> f32 {
        let target = (turn_dps.abs() / s.full_turn_dps)
            .max(move_mps.abs() / s.full_move_mps)
            .clamp(0.0, 1.0)
            * s.strength;
        if target > self.level {
            self.level = target; // clamp up instantly: late comfort is no comfort
        } else {
            self.level = (self.level - s.release_rate * dt).max(target);
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rises_instantly_and_relaxes() {
        let s = ComfortSettings::default();
        let mut v = Vignette::default();
        assert!((v.update(&s, 0.011, 120.0, 0.0) - 0.6).abs() < 1e-6);
        let l = v.update(&s, 0.1, 0.0, 0.0);
        assert!(l < 0.6 && l > 0.0);
        for _ in 0..100 {
            v.update(&s, 0.1, 0.0, 0.0);
        }
        assert_eq!(v.update(&s, 0.1, 0.0, 0.0), 0.0);
    }

    #[test]
    fn standing_still_never_vignettes() {
        let mut v = Vignette::default();
        assert_eq!(v.update(&ComfortSettings::default(), 0.011, 0.0, 0.0), 0.0);
    }
}
