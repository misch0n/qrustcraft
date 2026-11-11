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
        if world.solid(next) {
            // Landed in a solid: stand on top of the block that was hit.
            let feet = Vec3::new(p.x, next.y.floor() + 1.0, p.z);
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
