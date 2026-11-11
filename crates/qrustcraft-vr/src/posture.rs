//! Physical sneak, crawl and jump, from the head's height.
//!
//! Vivecraft keeps one calibrated standing height and compares against fixed metre thresholds,
//! and notes (in practice) that standing up from a squat can read as a jump. Improvements here:
//! thresholds are *fractions of the player's own height* (so a child and a tall adult both
//! sneak at a natural depth), and a jump must start from an upright stance, so rising out of a
//! crouch never fires one.

use crate::{Vec3, history::History};

#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    /// Head height above the floor when standing straight, metres.
    pub standing: f32,
    /// Shoulder-to-fingertip length, metres (0 = unknown; reach code falls back to 0.7).
    pub arm_length: f32,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            standing: 1.65,
            arm_length: 0.0,
        }
    }
}

impl Calibration {
    /// Calibrate from a short window of head heights taken while standing still.
    pub fn from_standing_samples(heights: &[f32]) -> Option<Self> {
        if heights.len() < 10 {
            return None;
        }
        let mut s = heights.to_vec();
        s.sort_by(f32::total_cmp);
        let median = s[s.len() / 2];
        (1.0..2.4).contains(&median).then_some(Self {
            standing: median,
            arm_length: 0.0,
        })
    }

    /// Arm length from a T-pose: the distance between the two hands is about 2 arms plus the
    /// shoulder width (0.4 m).
    pub fn with_t_pose(mut self, hand_span: f32) -> Self {
        self.arm_length = ((hand_span - 0.4) / 2.0).clamp(0.45, 0.95);
        self
    }

    pub fn arm(&self) -> f32 {
        if self.arm_length > 0.0 {
            self.arm_length
        } else {
            0.7
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PostureSettings {
    /// Sneak when the head is this fraction of standing height lower (Vivecraft: 0.4 m of 1.52).
    pub sneak_drop: f32,
    /// Crawl when the head is below this fraction of standing height (0.82 m of 1.52).
    pub crawl_fraction: f32,
    /// Jump: rise above standing this far, as a fraction of height.
    pub jump_above: f32,
    /// Jump: rise this far inside `jump_window`, fraction of height (0.1 m of 1.52).
    pub jump_rise: f32,
    pub jump_window: f32,
}

impl Default for PostureSettings {
    fn default() -> Self {
        Self {
            sneak_drop: 0.26,
            crawl_fraction: 0.54,
            jump_above: 0.03,
            jump_rise: 0.066,
            jump_window: 0.25,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Posture {
    pub sneak: bool,
    pub crawl: bool,
    pub jump: bool,
}

#[derive(Debug)]
pub struct PostureTracker {
    head_y: History,
    jump_latched: bool,
}

impl Default for PostureTracker {
    fn default() -> Self {
        Self {
            head_y: History::new(1.0),
            jump_latched: false,
        }
    }
}

impl PostureTracker {
    /// `head_y` is the head height in playspace (floor = 0), per fixed step `dt`.
    pub fn update(
        &mut self,
        cal: &Calibration,
        s: &PostureSettings,
        dt: f32,
        head_y: f32,
        on_ground: bool,
    ) -> Posture {
        self.head_y.push(dt, Vec3::new(0.0, head_y, 0.0));
        let h = cal.standing;
        let smooth = self.head_y.average_position(0.2).map_or(head_y, |v| v.y);

        let crawl = smooth < s.crawl_fraction * h;
        let sneak = !crawl && h - smooth > s.sneak_drop * h;

        // A hop: the head rises quickly *and* ends above standing, *and* the window began upright
        // (its lowest point never went below the sneak line). Standing up from a crouch fails the
        // last test.
        let (lo, hi) = self
            .head_y
            .extent(s.jump_window, 1)
            .unwrap_or((head_y, head_y));
        let rise = hi - lo;
        let upright_start = lo > h - 0.5 * s.sneak_drop * h;
        let hopping = rise > s.jump_rise * h && head_y > h + s.jump_above * h && upright_start;
        let jump = hopping && on_ground && !sneak && !self.jump_latched;
        if jump {
            self.jump_latched = true;
        } else if head_y < h + 0.5 * s.jump_above * h {
            self.jump_latched = false;
        }
        Posture { sneak, crawl, jump }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 90.0;

    fn drive(t: &mut PostureTracker, cal: &Calibration, heights: &[(f32, f32)]) -> Vec<Posture> {
        // (height, seconds) segments, linearly interpolated from the previous height.
        let s = PostureSettings::default();
        let mut out = vec![];
        let mut prev = cal.standing;
        for &(h, secs) in heights {
            let n = (secs / DT).max(1.0) as usize;
            for k in 1..=n {
                let y = prev + (h - prev) * k as f32 / n as f32;
                out.push(t.update(cal, &s, DT, y, true));
            }
            prev = h;
        }
        out
    }

    #[test]
    fn calibration_takes_the_median_and_rejects_nonsense() {
        let c = Calibration::from_standing_samples(&[1.7; 20]).unwrap();
        assert!((c.standing - 1.7).abs() < 1e-6);
        assert!(Calibration::from_standing_samples(&[1.7; 3]).is_none());
        assert!(Calibration::from_standing_samples(&[0.2; 20]).is_none());
        let c = c.with_t_pose(1.9);
        assert!((c.arm() - 0.75).abs() < 1e-5);
    }

    #[test]
    fn crouching_sneaks_and_scales_with_height() {
        for standing in [1.2f32, 1.65, 1.95] {
            let cal = Calibration {
                standing,
                arm_length: 0.0,
            };
            let mut t = PostureTracker::default();
            let p = drive(
                &mut t,
                &cal,
                &[
                    (standing, 0.3),
                    (standing * 0.7, 0.4),
                    (standing * 0.7, 0.4),
                ],
            );
            assert!(p.last().unwrap().sneak, "height {standing}");
            let mut t = PostureTracker::default();
            let p = drive(&mut t, &cal, &[(standing, 0.5)]);
            assert!(p.iter().all(|x| !x.sneak && !x.crawl));
        }
    }

    #[test]
    fn crawling_is_not_also_sneaking() {
        let cal = Calibration::default();
        let mut t = PostureTracker::default();
        let p = drive(&mut t, &cal, &[(cal.standing * 0.4, 0.8)]);
        let last = p.last().unwrap();
        assert!(last.crawl && !last.sneak);
    }

    #[test]
    fn a_hop_jumps_once() {
        let cal = Calibration::default();
        let mut t = PostureTracker::default();
        let p = drive(
            &mut t,
            &cal,
            &[
                (cal.standing, 0.3),
                (cal.standing + 0.14, 0.12),
                (cal.standing, 0.3),
            ],
        );
        assert_eq!(p.iter().filter(|x| x.jump).count(), 1);
    }

    #[test]
    fn standing_up_from_a_crouch_is_not_a_jump() {
        let cal = Calibration::default();
        let mut t = PostureTracker::default();
        // Crouch, then stand up fast and overshoot a little, which Vivecraft's rule can misread.
        let p = drive(
            &mut t,
            &cal,
            &[
                (cal.standing * 0.65, 0.5),
                (cal.standing + 0.06, 0.15),
                (cal.standing, 0.3),
            ],
        );
        assert!(p.iter().all(|x| !x.jump));
    }
}
