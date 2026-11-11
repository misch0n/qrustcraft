//! Teleport arc: a ballistic parabola from the hand, stepped against the world.
//!
//! The world query is a closure so this works with any collision source (Horizon's block
//! collision, a test grid). The destination must have standing room.

use crate::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct ArcConfig {
    pub speed: f32,
    pub gravity: f32,
    pub step: f32,
    pub max_steps: usize,
    pub max_distance: f32,
}

impl Default for ArcConfig {
    fn default() -> Self {
        Self {
            speed: 9.0,
            gravity: 9.8,
            step: 0.04,
            max_steps: 100,
            max_distance: 16.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arc {
    /// Valid landing spot (feet position).
    Valid {
        points: Vec<Vec3>,
        destination: Vec3,
    },
    /// Hit something unstandable, or ran out of range.
    Invalid { points: Vec<Vec3> },
}

/// What the world says about a point: solid? and, for a landing, the standable floor height.
pub trait World {
    fn solid(&self, p: Vec3) -> bool;
    /// Can a player stand with feet at `feet` (a 0.6 x 1.8 box is clear)?
    fn standable(&self, feet: Vec3) -> bool;
}

/// Longest stretch of the arc tested as one segment, so a fast arc cannot skip through a block.
const PROBE: f32 = 0.05;

/// Trace the arc. It lands only through the *top* of a block: hitting a block's side (a wall)
/// is invalid, because the floor beside a wall is somewhere below the hit, not level with it.
pub fn trace(world: &dyn World, origin: Vec3, direction: Vec3, cfg: &ArcConfig) -> Arc {
    let mut p = origin;
    let mut v = direction.normalize() * cfg.speed;
    let mut points = vec![p];
    for _ in 0..cfg.max_steps {
        let next_v = v + Vec3::new(0.0, -cfg.gravity * cfg.step, 0.0);
        let next = p + v * cfg.step;
        if (next - origin).length() > cfg.max_distance {
            return Arc::Invalid { points };
        }
        let n = ((next - p).length() / PROBE).ceil().max(1.0) as usize;
        let mut prev = p;
        for k in 1..=n {
            let q = p.lerp(next, k as f32 / n as f32);
            if !world.solid(q) {
                prev = q;
                continue;
            }
            points.push(q);
            let top = q.y.floor() + 1.0;
            if prev.y < top {
                // Came in through the side of the block.
                return Arc::Invalid { points };
            }
            // Through the top: stand on it, over the block that was hit.
            let feet = Vec3::new(q.x, top, q.z);
            points.push(feet);
            return if world.standable(feet) {
                Arc::Valid {
                    points,
                    destination: feet,
                }
            } else {
                Arc::Invalid { points }
            };
        }
        p = next;
        v = next_v;
        points.push(p);
    }
    Arc::Invalid { points }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat world: everything at y < 64 is solid; a pillar blocks standing at x in [10, 12).
    struct Flat;
    impl World for Flat {
        fn solid(&self, p: Vec3) -> bool {
            p.y < 64.0
        }
        fn standable(&self, feet: Vec3) -> bool {
            !(10.0..12.0).contains(&feet.x)
        }
    }

    #[test]
    fn lands_on_the_floor_ahead() {
        let a = trace(
            &Flat,
            Vec3::new(0.0, 65.5, 0.0),
            Vec3::new(0.0, 0.3, 1.0),
            &ArcConfig::default(),
        );
        match a {
            Arc::Valid { destination, .. } => {
                assert!((destination.y - 64.0).abs() < 1e-4);
                assert!(destination.z > 2.0, "{destination:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unstandable_target_is_invalid() {
        let a = trace(
            &Flat,
            Vec3::new(10.5, 65.5, -3.0),
            Vec3::new(0.0, 0.3, 1.0),
            &ArcConfig::default(),
        );
        assert!(matches!(a, Arc::Invalid { .. }));
    }

    /// Flat floor at y = 64 with a wall three blocks tall and thick across z in [5, 8).
    struct Wall;
    impl World for Wall {
        fn solid(&self, p: Vec3) -> bool {
            p.y < 64.0 || ((5.0..8.0).contains(&p.z) && p.y < 67.0)
        }
        fn standable(&self, feet: Vec3) -> bool {
            !self.solid(feet) && !self.solid(feet + Vec3::new(0.0, 1.0, 0.0))
        }
    }

    #[test]
    fn hitting_a_wall_side_is_invalid_not_a_floating_landing() {
        let a = trace(
            &Wall,
            Vec3::new(0.0, 65.5, 0.0),
            Vec3::new(0.0, 0.1, 1.0),
            &ArcConfig::default(),
        );
        assert!(matches!(a, Arc::Invalid { .. }), "{a:?}");
    }

    #[test]
    fn landing_on_top_of_the_wall_is_valid() {
        let a = trace(
            &Wall,
            Vec3::new(0.0, 66.5, 0.0),
            Vec3::new(0.0, 0.5, 0.866),
            &ArcConfig::default(),
        );
        match a {
            Arc::Valid { destination, .. } => {
                assert!((destination.y - 67.0).abs() < 1e-4, "{destination:?}");
                assert!((5.0..8.0).contains(&destination.z), "{destination:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_fast_falling_arc_lands_on_the_first_block_not_inside_one() {
        // Thrown hard downward from high up: each step drops well over a block.
        let cfg = ArcConfig {
            speed: 40.0,
            max_distance: 64.0,
            ..Default::default()
        };
        let a = trace(
            &Wall,
            Vec3::new(0.0, 80.0, 5.5),
            Vec3::new(0.0, -1.0, 0.0),
            &cfg,
        );
        match a {
            Arc::Valid { destination, .. } => {
                assert!((destination.y - 67.0).abs() < 1e-4, "{destination:?}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn aiming_at_the_sky_runs_out_of_range() {
        let cfg = ArcConfig {
            gravity: 0.0,
            ..Default::default()
        };
        let a = trace(
            &Flat,
            Vec3::new(0.0, 65.5, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            &cfg,
        );
        assert!(matches!(a, Arc::Invalid { .. }));
    }
}
