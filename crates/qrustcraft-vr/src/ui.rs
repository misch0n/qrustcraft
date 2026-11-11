//! The floating GUI panel: Minecraft's 2D screens (inventory, menus, chat) shown on a quad in the
//! world and operated with a controller ray, which becomes a normal 2D cursor.
//!
//! Horizon already registers its widgets for controllers (`ui::button` and friends), so the panel
//! only has to produce a cursor position in GUI pixels and press/release events.

use crate::{Pose, Quat, Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct GuiPanel {
    /// Centre of the panel in world space.
    pub center: Vec3,
    /// Orientation: the panel faces its local +Z.
    pub orientation: Quat,
    /// Width in metres; height follows the GUI aspect.
    pub width_m: f32,
    /// GUI size in pixels.
    pub gui_px: Vec2,
}

impl GuiPanel {
    pub fn height_m(&self) -> f32 {
        self.width_m * self.gui_px.y / self.gui_px.x
    }

    /// Place the panel `distance` metres in front of the head, facing it, level with the horizon
    /// (pitch ignored, so it doesn't tilt with your gaze when it opens).
    pub fn in_front_of(head: &Pose, distance: f32, width_m: f32, gui_px: Vec2) -> Self {
        let mut f = head.forward();
        f.y = 0.0;
        let f = if f.length() < 1e-4 {
            Vec3::NEG_Z
        } else {
            f.normalize()
        };
        let center = head.position + f * distance;
        // The quad's +Z must point back at the player.
        let orientation = Quat::from_rotation_arc(Vec3::Z, -f);
        Self {
            center,
            orientation,
            width_m,
            gui_px,
        }
    }

    /// Intersect a ray with the panel. Returns the cursor in GUI pixels (origin top-left) and the
    /// hit distance, or `None` if the ray misses or points away.
    pub fn hit(&self, origin: Vec3, direction: Vec3) -> Option<(Vec2, f32)> {
        let normal = self.orientation * Vec3::Z;
        let denom = normal.dot(direction);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = normal.dot(self.center - origin) / denom;
        if t <= 0.0 {
            return None;
        }
        let local = self.orientation.inverse() * (origin + direction * t - self.center);
        let (hw, hh) = (self.width_m / 2.0, self.height_m() / 2.0);
        if local.x.abs() > hw || local.y.abs() > hh {
            return None;
        }
        let px = Vec2::new(
            (local.x / hw * 0.5 + 0.5) * self.gui_px.x,
            (0.5 - local.y / hh * 0.5) * self.gui_px.y,
        );
        Some((px, t))
    }
}

/// Press/release edges for the cursor, from the pointing hand's trigger.
#[derive(Debug, Default)]
pub struct Cursor {
    down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CursorEvent {
    Move(Vec2),
    Press(Vec2),
    Release(Vec2),
}

impl Cursor {
    pub fn update(&mut self, at: Option<Vec2>, pressed: bool) -> Vec<CursorEvent> {
        let mut out = vec![];
        if let Some(p) = at {
            out.push(CursorEvent::Move(p));
            if pressed && !self.down {
                out.push(CursorEvent::Press(p));
            }
            if !pressed && self.down {
                out.push(CursorEvent::Release(p));
            }
            self.down = pressed;
        } else if self.down && !pressed {
            // Released off-panel: don't leave a button stuck.
            self.down = false;
            out.push(CursorEvent::Release(Vec2::ZERO));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> GuiPanel {
        // 2 m wide, 16:9, centred 2 m south of the origin, facing north (towards the player).
        GuiPanel {
            center: Vec3::new(0.0, 1.6, 2.0),
            orientation: Quat::from_rotation_y(std::f32::consts::PI),
            width_m: 2.0,
            gui_px: Vec2::new(1920.0, 1080.0),
        }
    }

    #[test]
    fn centre_ray_hits_centre_pixel() {
        let (px, t) = panel().hit(Vec3::new(0.0, 1.6, 0.0), Vec3::Z).unwrap();
        assert!((px - Vec2::new(960.0, 540.0)).length() < 1e-2, "{px:?}");
        assert!((t - 2.0).abs() < 1e-5);
    }

    #[test]
    fn up_is_smaller_y_pixel() {
        let h = panel().height_m();
        let (px, _) = panel()
            .hit(Vec3::new(0.0, 1.6 + h / 4.0, 0.0), Vec3::Z)
            .unwrap();
        assert!((px.y - 270.0).abs() < 1e-2, "{px:?}");
    }

    #[test]
    fn misses_and_backfaces() {
        assert!(panel().hit(Vec3::new(5.0, 1.6, 0.0), Vec3::Z).is_none());
        assert!(panel().hit(Vec3::new(0.0, 1.6, 0.0), Vec3::NEG_Z).is_none());
    }

    #[test]
    fn in_front_faces_the_player_and_ignores_pitch() {
        let head = Pose::new(Vec3::new(0.0, 1.7, 0.0), Quat::from_rotation_x(-0.5)); // looking down
        let p = GuiPanel::in_front_of(&head, 2.0, 2.0, Vec2::new(800.0, 450.0));
        assert!((p.center - Vec3::new(0.0, 1.7, -2.0)).length() < 1e-4);
        assert!(p.hit(head.position, Vec3::NEG_Z).is_some());
    }

    #[test]
    fn cursor_edges() {
        let mut c = Cursor::default();
        let p = Vec2::new(10.0, 10.0);
        assert_eq!(
            c.update(Some(p), true),
            vec![CursorEvent::Move(p), CursorEvent::Press(p)]
        );
        assert_eq!(c.update(Some(p), true), vec![CursorEvent::Move(p)]);
        assert_eq!(
            c.update(Some(p), false),
            vec![CursorEvent::Move(p), CursorEvent::Release(p)]
        );
        c.update(Some(p), true);
        assert_eq!(
            c.update(None, false),
            vec![CursorEvent::Release(Vec2::ZERO)]
        );
    }
}
