//! Controller actions: the Oculus Touch profile (Quest 2/3/Pro) plus a generic fallback.

use crate::{Result, convert};
use openxr as xr;
use qrustcraft_vr::{Hand, Pose, Vec2, haptics::Pulse, input::ControllerState};

pub struct Controllers {
    set: xr::ActionSet,
    grip_pose: xr::Action<xr::Posef>,
    _aim_pose: xr::Action<xr::Posef>,
    stick: xr::Action<xr::Vector2f>,
    stick_click: xr::Action<bool>,
    trigger: xr::Action<f32>,
    grip: xr::Action<f32>,
    primary: xr::Action<bool>,
    secondary: xr::Action<bool>,
    menu: xr::Action<bool>,
    haptic: xr::Action<xr::Haptic>,
    paths: [xr::Path; 2],
    grip_spaces: [xr::Space; 2],
    aim_spaces: [xr::Space; 2],
}

/// Poses located this frame, in playspace (the reference space the loop was built with).
#[derive(Clone, Copy, Debug, Default)]
pub struct HandPoses {
    /// The pose of the hand/grip (where the tool is held).
    pub grip: [Option<Pose>; 2],
    /// The runtime's pointing pose (for UI rays).
    pub aim: [Option<Pose>; 2],
}

fn bind<'a, T: xr::ActionTy>(
    instance: &xr::Instance,
    a: &'a xr::Action<T>,
    path: &str,
) -> Result<xr::Binding<'a>> {
    Ok(xr::Binding::new(a, instance.string_to_path(path)?))
}

fn idx(h: Hand) -> usize {
    match h {
        Hand::Left => 0,
        Hand::Right => 1,
    }
}

impl Controllers {
    pub fn new<G: xr::Graphics>(instance: &xr::Instance, session: &xr::Session<G>) -> Result<Self> {
        let set = instance.create_action_set("gameplay", "Gameplay", 0)?;
        let paths = [
            instance.string_to_path("/user/hand/left")?,
            instance.string_to_path("/user/hand/right")?,
        ];
        let both = &paths[..];
        let grip_pose = set.create_action::<xr::Posef>("grip_pose", "Hand grip", both)?;
        let aim_pose = set.create_action::<xr::Posef>("aim_pose", "Hand aim", both)?;
        let stick = set.create_action::<xr::Vector2f>("stick", "Thumbstick", both)?;
        let stick_click = set.create_action::<bool>("stick_click", "Thumbstick click", both)?;
        let trigger = set.create_action::<f32>("trigger", "Trigger", both)?;
        let grip = set.create_action::<f32>("grip", "Grip", both)?;
        let primary = set.create_action::<bool>("primary", "A / X", both)?;
        let secondary = set.create_action::<bool>("secondary", "B / Y", both)?;
        let menu = set.create_action::<bool>("menu", "Menu", both)?;
        let haptic = set.create_action::<xr::Haptic>("haptic", "Haptic", both)?;

        let p = |s: &str| instance.string_to_path(s);
        macro_rules! b {
            ($a:expr, $s:expr) => {
                bind(instance, $a, $s)
            };
        }
        // Oculus Touch: the profile Quest 2, 3 and Pro all report.
        let mut touch = vec![];
        for side in ["left", "right"] {
            let base = format!("/user/hand/{side}/input");
            touch.push(b!(&grip_pose, &format!("{base}/grip/pose"))?);
            touch.push(b!(&aim_pose, &format!("{base}/aim/pose"))?);
            touch.push(b!(&stick, &format!("{base}/thumbstick"))?);
            touch.push(b!(&stick_click, &format!("{base}/thumbstick/click"))?);
            touch.push(b!(&trigger, &format!("{base}/trigger/value"))?);
            touch.push(b!(&grip, &format!("{base}/squeeze/value"))?);
            touch.push(b!(&haptic, &format!("/user/hand/{side}/output/haptic"))?);
        }
        // A/B on the right, X/Y on the left; menu only exists on the left.
        touch.push(b!(&primary, "/user/hand/right/input/a/click")?);
        touch.push(b!(&secondary, "/user/hand/right/input/b/click")?);
        touch.push(b!(&primary, "/user/hand/left/input/x/click")?);
        touch.push(b!(&secondary, "/user/hand/left/input/y/click")?);
        touch.push(b!(&menu, "/user/hand/left/input/menu/click")?);
        instance.suggest_interaction_profile_bindings(
            p("/interaction_profiles/oculus/touch_controller")?,
            &touch,
        )?;

        // Khronos simple controller: every runtime has it, so something always works.
        let mut simple = vec![];
        for side in ["left", "right"] {
            simple.push(b!(
                &grip_pose,
                &format!("/user/hand/{side}/input/grip/pose")
            )?);
            simple.push(b!(&aim_pose, &format!("/user/hand/{side}/input/aim/pose"))?);
            simple.push(b!(
                &primary,
                &format!("/user/hand/{side}/input/select/click")
            )?);
            simple.push(b!(&menu, &format!("/user/hand/{side}/input/menu/click"))?);
            simple.push(b!(&haptic, &format!("/user/hand/{side}/output/haptic"))?);
        }
        instance.suggest_interaction_profile_bindings(
            p("/interaction_profiles/khr/simple_controller")?,
            &simple,
        )?;

        session.attach_action_sets(&[&set])?;
        let space =
            |a: &xr::Action<xr::Posef>, path| a.create_space(session, path, xr::Posef::IDENTITY);
        let grip_spaces = [space(&grip_pose, paths[0])?, space(&grip_pose, paths[1])?];
        let aim_spaces = [space(&aim_pose, paths[0])?, space(&aim_pose, paths[1])?];
        Ok(Self {
            set,
            grip_pose,
            _aim_pose: aim_pose,
            stick,
            stick_click,
            trigger,
            grip,
            primary,
            secondary,
            menu,
            haptic,
            paths,
            grip_spaces,
            aim_spaces,
        })
    }

    /// Sync and read both hands. Call once per frame, after `wait_frame`.
    pub fn poll<G: xr::Graphics>(
        &self,
        session: &xr::Session<G>,
        space: &xr::Space,
        time: xr::Time,
    ) -> Result<([ControllerState; 2], HandPoses)> {
        session.sync_actions(&[(&self.set).into()])?;
        let mut states = [ControllerState::default(); 2];
        let mut poses = HandPoses::default();
        for i in 0..2 {
            let path = self.paths[i];
            let tracked = self.grip_pose.is_active(session, path)?;
            let s = self.stick.state(session, path)?.current_state;
            states[i] = ControllerState {
                tracked,
                stick: Vec2::new(s.x, s.y),
                stick_click: self.stick_click.state(session, path)?.current_state,
                trigger: self.trigger.state(session, path)?.current_state,
                grip: self.grip.state(session, path)?.current_state,
                primary: self.primary.state(session, path)?.current_state,
                secondary: self.secondary.state(session, path)?.current_state,
                menu: self.menu.state(session, path)?.current_state,
            };
            let valid = |f: xr::SpaceLocationFlags| {
                f.contains(
                    xr::SpaceLocationFlags::POSITION_VALID
                        | xr::SpaceLocationFlags::ORIENTATION_VALID,
                )
            };
            if tracked {
                let g = self.grip_spaces[i].locate(space, time)?;
                let a = self.aim_spaces[i].locate(space, time)?;
                poses.grip[i] = valid(g.location_flags).then(|| convert::pose(g.pose));
                poses.aim[i] = valid(a.location_flags).then(|| convert::pose(a.pose));
            }
        }
        Ok((states, poses))
    }

    pub fn vibrate<G: xr::Graphics>(&self, session: &xr::Session<G>, pulse: Pulse) -> Result<()> {
        self.haptic.apply_feedback(
            session,
            self.paths[idx(pulse.hand)],
            &xr::HapticVibration::new()
                .amplitude(pulse.amplitude)
                .duration(xr::Duration::from_nanos(
                    i64::from(pulse.duration_ms) * 1_000_000,
                ))
                .frequency(xr::FREQUENCY_UNSPECIFIED),
        )?;
        Ok(())
    }
}
