//! Fixed-step pose history: the one gesture primitive.
//!
//! Vivecraft stamps samples with wall-clock time per poll, so its gesture thresholds drift with
//! frame rate. Here every sample carries *simulation* time advanced by explicit `dt`, so a
//! "speed over 0.33 s" means the same at 72 Hz and 120 Hz, and tests are deterministic.
//!
//! The clock is `f64`: it only ever grows, and an `f32` one loses a frame's worth of precision
//! within hours (at 2^17 s its step is 1/64 s), which would quantise every speed it measures.

use crate::Vec3;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct History {
    samples: VecDeque<(f64, Vec3)>,
    now: f64,
    keep: f64,
}

impl History {
    /// Keep at least `keep_secs` of samples.
    pub fn new(keep_secs: f32) -> Self {
        Self {
            samples: VecDeque::new(),
            now: 0.0,
            keep: f64::from(keep_secs),
        }
    }

    pub fn push(&mut self, dt: f32, v: Vec3) {
        self.now += f64::from(dt);
        self.samples.push_back((self.now, v));
        while self
            .samples
            .front()
            .is_some_and(|(t, _)| *t < self.now - self.keep)
        {
            self.samples.pop_front();
        }
    }

    /// Forget every sample (tracking was lost: motion across the gap is not real motion).
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn latest(&self) -> Option<Vec3> {
        self.samples.back().map(|s| s.1)
    }

    fn window(&self, secs: f32) -> impl Iterator<Item = &(f64, Vec3)> {
        let from = self.now - f64::from(secs);
        self.samples.iter().filter(move |(t, _)| *t >= from)
    }

    /// Mean position over the last `secs`.
    pub fn average_position(&self, secs: f32) -> Option<Vec3> {
        let (sum, n) = self
            .window(secs)
            .fold((Vec3::ZERO, 0u32), |(s, n), (_, v)| (s + *v, n + 1));
        (n > 0).then(|| sum / n as f32)
    }

    /// Displacement from the oldest sample in the window to the newest.
    pub fn net_movement(&self, secs: f32) -> Vec3 {
        let mut it = self.window(secs);
        match (it.next(), self.samples.back()) {
            (Some(first), Some(last)) => last.1 - first.1,
            _ => Vec3::ZERO,
        }
    }

    /// Mean speed (path length over elapsed time) over the last `secs`, in units per second.
    pub fn average_speed(&self, secs: f32) -> f32 {
        let pts: Vec<_> = self.window(secs).collect();
        if pts.len() < 2 {
            return 0.0;
        }
        let path: f32 = pts.windows(2).map(|w| (w[1].1 - w[0].1).length()).sum();
        let dt = (pts[pts.len() - 1].0 - pts[0].0) as f32;
        if dt > 0.0 { path / dt } else { 0.0 }
    }

    /// Lowest and highest value of `axis` (0=x, 1=y, 2=z) in the window.
    pub fn extent(&self, secs: f32, axis: usize) -> Option<(f32, f32)> {
        self.window(secs)
            .map(|(_, v)| v[axis])
            .fold(None, |acc, x| match acc {
                None => Some((x, x)),
                Some((lo, hi)) => Some((lo.min(x), hi.max(x))),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(hz: f32, secs: f32, speed: f32) -> History {
        let mut h = History::new(1.0);
        let dt = 1.0 / hz;
        for k in 0..=(secs * hz) as usize {
            h.push(dt, Vec3::new(speed * k as f32 * dt, 0.0, 0.0));
        }
        h
    }

    #[test]
    fn speed_is_independent_of_frame_rate() {
        for hz in [72.0, 90.0, 120.0] {
            let s = run(hz, 1.0, 3.0).average_speed(0.33);
            assert!((s - 3.0).abs() < 0.05, "{hz} Hz gave {s}");
        }
    }

    #[test]
    fn old_samples_fall_out_of_the_window() {
        let mut h = run(90.0, 0.5, 5.0);
        for _ in 0..90 {
            h.push(1.0 / 90.0, h.latest().unwrap()); // stop
        }
        assert!(h.average_speed(0.33) < 1e-4);
    }

    #[test]
    fn speed_stays_exact_after_hours_of_play() {
        let mut h = History::new(1.0);
        h.push(6.0 * 3600.0, Vec3::ZERO); // six hours in
        let dt = 1.0 / 90.0;
        for k in 1..=90 {
            h.push(dt, Vec3::new(3.0 * k as f32 * dt, 0.0, 0.0));
        }
        let s = h.average_speed(0.33);
        assert!((s - 3.0).abs() < 0.05, "{s}");
    }

    #[test]
    fn clear_forgets_motion() {
        let mut h = run(90.0, 0.5, 5.0);
        h.clear();
        assert!(h.latest().is_none());
        h.push(1.0 / 90.0, Vec3::new(100.0, 0.0, 0.0));
        assert_eq!(h.average_speed(0.33), 0.0);
    }

    #[test]
    fn net_movement_and_average_and_extent() {
        let h = run(100.0, 1.0, 2.0);
        assert!((h.net_movement(0.3).x - 0.6).abs() < 0.03);
        assert!(h.average_position(0.3).unwrap().x > 1.5);
        let (lo, hi) = h.extent(0.3, 0).unwrap();
        assert!(hi > lo);
        assert!(History::new(1.0).average_position(0.3).is_none());
    }
}
