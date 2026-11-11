//! Controller state -> game intents.
//!
//! The XR layer fills a [`ControllerState`] per hand each frame (this matches the Quest Touch
//! profile; other profiles map onto the same fields). [`IntentMapper`] turns two of them into
//! the [`Intent`] the game applies, handling deadzones, edge detection and the left/right-handed
//! swap. Mining and attacking are *not* here: they come from [`crate::interact`].

use crate::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControllerState {
    pub tracked: bool,
    pub stick: Vec2,
    pub stick_click: bool,
    /// Analog trigger 0..=1.
    pub trigger: f32,
    /// Analog grip 0..=1.
    pub grip: f32,
    /// A / X.
    pub primary: bool,
    /// B / Y.
    pub secondary: bool,
    /// Menu (left) / system-reserved (right).
    pub menu: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Bindings {
    pub right_handed: bool,
    pub deadzone: f32,
    /// Squeeze needed to count as pressed, with hysteresis `press`/`release`.
    pub press: f32,
    pub release: f32,
    pub snap_turn: bool,
    pub snap_degrees: f32,
    pub smooth_turn_dps: f32,
    /// Walk relative to the head, not the hand.
    pub move_relative_to_head: bool,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            right_handed: true,
            deadzone: 0.2,
            press: 0.75,
            release: 0.55,
            snap_turn: true,
            snap_degrees: 45.0,
            smooth_turn_dps: 120.0,
            move_relative_to_head: true,
        }
    }
}

/// What the game should do this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    /// x = strafe right, y = forward; each -1..=1.
    pub movement: Vec2,
    /// Rig yaw change this frame, degrees (positive clockwise from above).
    pub turn_deg: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    /// Use item / place block (held; the game does its own edge handling).
    pub use_item: bool,
    pub use_item_pressed: bool,
    /// Held: mining/attack engagement is decided by `interact`, this is the "tool grabbed" state.
    pub grab: bool,
    /// Hotbar slot delta: -1 or +1 on the frame it triggers.
    pub hotbar_step: i32,
    pub swap_hands: bool,
    pub drop_item: bool,
    pub open_inventory: bool,
    pub open_menu: bool,
    /// Teleport arc held (stick pushed forward on the off hand in teleport mode).
    pub teleport_aim: bool,
    pub teleport_release: bool,
}

/// Remembers the last frame for edges, hysteresis and snap-turn latching.
#[derive(Debug, Default)]
pub struct IntentMapper {
    trigger_down: bool,
    grip_down: [bool; 2],
    sprint: bool,
    snap_latched: bool,
    hotbar_latched: bool,
    prev_primary: [bool; 2],
    prev_secondary: [bool; 2],
    prev_menu: bool,
    prev_aim: bool,
}

fn deadzone(v: Vec2, dz: f32) -> Vec2 {
    let m = v.length();
    if m <= dz {
        Vec2::ZERO
    } else {
        v / m * ((m - dz) / (1.0 - dz)).min(1.0)
    }
}

impl IntentMapper {
    pub fn update(
        &mut self,
        b: &Bindings,
        left: &ControllerState,
        right: &ControllerState,
        dt: f32,
        teleport_mode: bool,
    ) -> Intent {
        let (main, off) = if b.right_handed {
            (right, left)
        } else {
            (left, right)
        };
        let main_i = usize::from(b.right_handed);
        let off_i = 1 - main_i;
        let mut i = Intent::default();

        // Off hand stick: locomotion. Main hand stick: turn (X), hotbar (Y flick).
        let ms = if off.tracked {
            deadzone(off.stick, b.deadzone)
        } else {
            Vec2::ZERO
        };
        let ts = if main.tracked {
            deadzone(main.stick, b.deadzone)
        } else {
            Vec2::ZERO
        };

        if teleport_mode {
            let aiming = ms.y > 0.6 || (self.prev_aim && ms.y > 0.3);
            i.teleport_aim = aiming;
            i.teleport_release = self.prev_aim && !aiming;
            self.prev_aim = aiming;
        } else {
            i.movement = Vec2::new(ms.x, ms.y);
            self.prev_aim = false;
        }

        // Sprint: click the off stick to toggle, cleared when you stop moving.
        if off.stick_click && !self.sprint && i.movement.y > 0.5 {
            self.sprint = true;
        }
        if i.movement.length() < 0.1 {
            self.sprint = false;
        }
        i.sprint = self.sprint;

        // Turning.
        if b.snap_turn {
            if ts.x.abs() > 0.7 {
                if !self.snap_latched {
                    i.turn_deg = b.snap_degrees * ts.x.signum();
                    self.snap_latched = true;
                }
            } else if ts.x.abs() < 0.4 {
                self.snap_latched = false;
            }
        } else {
            i.turn_deg = ts.x * b.smooth_turn_dps * dt;
        }

        // Hotbar: main stick flicked up/down.
        if ts.y.abs() > 0.8 {
            if !self.hotbar_latched {
                i.hotbar_step = if ts.y > 0.0 { -1 } else { 1 };
                self.hotbar_latched = true;
            }
        } else if ts.y.abs() < 0.4 {
            self.hotbar_latched = false;
        }

        // Use item: main trigger with hysteresis.
        let was = self.trigger_down;
        self.trigger_down = if self.trigger_down {
            main.trigger > b.release
        } else {
            main.trigger > b.press
        };
        i.use_item = self.trigger_down;
        i.use_item_pressed = self.trigger_down && !was;

        // Grab (tool "held" hint for interact): main grip with hysteresis.
        for (k, c) in [left, right].into_iter().enumerate() {
            self.grip_down[k] = if self.grip_down[k] {
                c.grip > b.release
            } else {
                c.grip > b.press
            };
        }
        i.grab = self.grip_down[main_i];
        // Sneak: off-hand grip held.
        i.sneak = self.grip_down[off_i];

        // Buttons: main A = jump, main B = inventory, off A = swap hands, off B = drop, off menu = pause.
        let edge = |now: bool, prev: bool| now && !prev;
        i.jump = main.primary;
        i.open_inventory = edge(main.secondary, self.prev_secondary[main_i]);
        i.swap_hands = edge(off.primary, self.prev_primary[off_i]);
        i.drop_item = edge(off.secondary, self.prev_secondary[off_i]);
        i.open_menu = edge(left.menu, self.prev_menu);

        self.prev_primary = [left.primary, right.primary];
        self.prev_secondary = [left.secondary, right.secondary];
        self.prev_menu = left.menu;
        i
    }
}

/// Which way "forward" is for the stick: the head's yaw or a hand's, given in game yaw degrees.
/// Returns the world-space movement vector (x east, z south) for an input in `Intent::movement`.
pub fn movement_to_world(movement: Vec2, facing_yaw_deg: f32) -> Vec2 {
    let y = facing_yaw_deg.to_radians();
    let fwd = Vec2::new(-y.sin(), y.cos());
    let right = Vec2::new(-y.cos(), -y.sin()); // facing south: right is west
    fwd * movement.y + right * movement.x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stick(x: f32, y: f32) -> ControllerState {
        ControllerState {
            tracked: true,
            stick: Vec2::new(x, y),
            ..Default::default()
        }
    }

    #[test]
    fn deadzone_rescales() {
        assert_eq!(deadzone(Vec2::new(0.1, 0.0), 0.2), Vec2::ZERO);
        let v = deadzone(Vec2::new(1.0, 0.0), 0.2);
        assert!((v.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn off_hand_stick_walks() {
        let mut m = IntentMapper::default();
        let i = m.update(
            &Bindings::default(),
            &stick(0.0, 1.0),
            &stick(0.0, 0.0),
            0.011,
            false,
        );
        assert!(i.movement.y > 0.99);
    }

    #[test]
    fn left_handed_swaps_roles() {
        let mut m = IntentMapper::default();
        let b = Bindings {
            right_handed: false,
            ..Default::default()
        };
        let i = m.update(&b, &stick(0.0, 0.0), &stick(0.0, 1.0), 0.011, false);
        assert!(i.movement.y > 0.99);
    }

    #[test]
    fn snap_turn_fires_once_per_flick() {
        let mut m = IntentMapper::default();
        let b = Bindings::default();
        let mut total = 0.0;
        for _ in 0..30 {
            total += m
                .update(&b, &stick(0.0, 0.0), &stick(1.0, 0.0), 0.011, false)
                .turn_deg;
        }
        assert_eq!(total, 45.0);
        m.update(&b, &stick(0.0, 0.0), &stick(0.0, 0.0), 0.011, false); // release
        assert_eq!(
            m.update(&b, &stick(0.0, 0.0), &stick(-1.0, 0.0), 0.011, false)
                .turn_deg,
            -45.0
        );
    }

    #[test]
    fn smooth_turn_scales_with_dt() {
        let mut m = IntentMapper::default();
        let b = Bindings {
            snap_turn: false,
            ..Default::default()
        };
        let t = m
            .update(&b, &stick(0.0, 0.0), &stick(1.0, 0.0), 0.5, false)
            .turn_deg;
        assert!((t - 60.0).abs() < 1e-3);
    }

    #[test]
    fn trigger_hysteresis_avoids_chatter() {
        let mut m = IntentMapper::default();
        let b = Bindings::default();
        let trig = |v| ControllerState {
            tracked: true,
            trigger: v,
            ..Default::default()
        };
        let l = stick(0.0, 0.0);
        assert!(!m.update(&b, &l, &trig(0.7), 0.01, false).use_item);
        let i = m.update(&b, &l, &trig(0.8), 0.01, false);
        assert!(i.use_item && i.use_item_pressed);
        let i = m.update(&b, &l, &trig(0.6), 0.01, false);
        assert!(i.use_item && !i.use_item_pressed); // still held above release
        assert!(!m.update(&b, &l, &trig(0.5), 0.01, false).use_item);
    }

    #[test]
    fn hotbar_flick_steps_once() {
        let mut m = IntentMapper::default();
        let b = Bindings::default();
        let a = m
            .update(&b, &stick(0.0, 0.0), &stick(0.0, 1.0), 0.01, false)
            .hotbar_step;
        let c = m
            .update(&b, &stick(0.0, 0.0), &stick(0.0, 1.0), 0.01, false)
            .hotbar_step;
        assert_eq!((a, c), (-1, 0));
    }

    #[test]
    fn sprint_toggles_on_click_and_clears_on_stop() {
        let mut m = IntentMapper::default();
        let b = Bindings::default();
        let mut l = stick(0.0, 1.0);
        l.stick_click = true;
        assert!(m.update(&b, &l, &stick(0.0, 0.0), 0.01, false).sprint);
        l.stick_click = false;
        assert!(m.update(&b, &l, &stick(0.0, 0.0), 0.01, false).sprint);
        assert!(
            !m.update(&b, &stick(0.0, 0.0), &stick(0.0, 0.0), 0.01, false)
                .sprint
        );
    }

    #[test]
    fn teleport_mode_swallows_movement_and_releases() {
        let mut m = IntentMapper::default();
        let b = Bindings::default();
        let i = m.update(&b, &stick(0.0, 1.0), &stick(0.0, 0.0), 0.01, true);
        assert!(i.teleport_aim && i.movement == Vec2::ZERO);
        let i = m.update(&b, &stick(0.0, 0.0), &stick(0.0, 0.0), 0.01, true);
        assert!(i.teleport_release);
    }

    #[test]
    fn movement_maps_to_world() {
        // Facing south (yaw 0), forward is +Z, right is -X.
        let f = movement_to_world(Vec2::new(0.0, 1.0), 0.0);
        assert!((f - Vec2::new(0.0, 1.0)).length() < 1e-5);
        let r = movement_to_world(Vec2::new(1.0, 0.0), 0.0);
        assert!((r - Vec2::new(-1.0, 0.0)).length() < 1e-5);
        // Facing west (yaw 90), forward is -X.
        let f = movement_to_world(Vec2::new(0.0, 1.0), 90.0);
        assert!((f - Vec2::new(-1.0, 0.0)).length() < 1e-5);
    }
}
