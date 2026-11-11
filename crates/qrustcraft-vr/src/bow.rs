//! Two-handed bow: the bow hand holds, the arrow hand draws.
//!
//! Geometry follows Vivecraft (string point half a draw-length up the bow hand, notch within
//! 0.15 m and 20 degrees, aim from arrow hand toward bow hand) but is scaled by the player's own
//! arm length instead of a fixed fraction of the hitbox, and the result is an analog tension
//! 0..1 for the game to map onto its own charge stages.

use crate::{Pose, Vec3};

pub const NOTCH_RADIUS: f32 = 0.15;
pub const NOTCH_ANGLE_DEG: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    /// 0..=1 of the maximum draw.
    pub tension: f32,
    /// Unit vector the arrow flies along.
    pub aim: Vec3,
    pub notched: bool,
}

#[derive(Debug, Default)]
pub struct Bow {
    notched: bool,
}

impl Bow {
    /// `max_draw` metres (a third of the arm length reads well); both poses in world space.
    pub fn update(&mut self, bow_hand: &Pose, arrow_hand: &Pose, max_draw: f32) -> Option<Draw> {
        let string = bow_hand.position + bow_hand.up() * (max_draw * 0.5);
        let gap = arrow_hand.position.distance(string);
        let aligned = bow_hand
            .forward()
            .angle_between(arrow_hand.forward())
            .to_degrees()
            < NOTCH_ANGLE_DEG;
        if !self.notched {
            if gap < NOTCH_RADIUS && aligned {
                self.notched = true;
            } else {
                return None;
            }
        } else if gap > NOTCH_RADIUS + max_draw * 1.3 {
            // Pulled implausibly far: tracking glitch or the player let go.
            self.notched = false;
            return None;
        }
        let draw =
            (arrow_hand.position.distance(bow_hand.position) - NOTCH_RADIUS).clamp(0.0, max_draw);
        let aim = (bow_hand.position - arrow_hand.position)
            .try_normalize()
            .unwrap_or(bow_hand.forward());
        Some(Draw {
            tension: draw / max_draw,
            aim,
            notched: true,
        })
    }

    pub fn release(&mut self) {
        self.notched = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quat;

    fn bow_hand() -> Pose {
        Pose::new(Vec3::new(0.0, 1.3, -0.4), Quat::IDENTITY)
    }

    #[test]
    fn far_hand_does_not_notch() {
        let mut b = Bow::default();
        assert!(
            b.update(
                &bow_hand(),
                &Pose::new(Vec3::new(0.5, 1.3, 0.0), Quat::IDENTITY),
                0.3
            )
            .is_none()
        );
    }

    #[test]
    fn notch_then_pull_back_raises_tension_and_aims_forward() {
        let mut b = Bow::default();
        let bh = bow_hand();
        let string = bh.position + bh.up() * 0.15;
        assert!(
            b.update(&bh, &Pose::new(string, Quat::IDENTITY), 0.3)
                .is_some()
        );
        let near = b
            .update(
                &bh,
                &Pose::new(bh.position + Vec3::new(0.0, 0.0, 0.25), Quat::IDENTITY),
                0.3,
            )
            .unwrap();
        let far = b
            .update(
                &bh,
                &Pose::new(bh.position + Vec3::new(0.0, 0.0, 0.5), Quat::IDENTITY),
                0.3,
            )
            .unwrap();
        assert!(far.tension > near.tension && far.tension <= 1.0);
        assert!((far.aim - Vec3::NEG_Z).length() < 1e-4, "{:?}", far.aim);
    }

    #[test]
    fn misaligned_hands_do_not_notch() {
        let mut b = Bow::default();
        let bh = bow_hand();
        let sideways = Quat::from_rotation_y(1.2);
        assert!(
            b.update(&bh, &Pose::new(bh.position + bh.up() * 0.15, sideways), 0.3)
                .is_none()
        );
    }

    #[test]
    fn release_resets() {
        let mut b = Bow::default();
        let bh = bow_hand();
        b.update(
            &bh,
            &Pose::new(bh.position + bh.up() * 0.15, Quat::IDENTITY),
            0.3,
        );
        b.release();
        assert!(
            b.update(
                &bh,
                &Pose::new(Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY),
                0.3
            )
            .is_none()
        );
    }
}
