//! The OpenXR frame loop: events, wait/begin/locate/end.
//!
//! The graphics API stays generic: the Vulkan bridge supplies the swapchain, the loop only moves
//! frames.

use crate::{Result, context::VIEW_TYPE, convert};
use openxr as xr;
use qrustcraft_vr::{Pose, rig::EyeInput};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// Nothing the game must act on.
    None,
    /// The headset was taken off or the app lost focus: pause input and mute the world.
    Unfocused,
    Focused,
    /// Exit requested by the system (e.g. the Quest's "quit app").
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub struct FrameTiming {
    pub predicted_display_time: xr::Time,
    pub predicted_period: xr::Duration,
    pub should_render: bool,
}

pub struct FrameLoop<G: xr::Graphics> {
    pub session: xr::Session<G>,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<G>,
    pub space: xr::Space,
    pub blend_mode: xr::EnvironmentBlendMode,
    running: bool,
    focused: bool,
    events: xr::EventDataBuffer,
}

/// What `begin` hands the game for one frame.
pub struct FrameState {
    pub timing: FrameTiming,
    /// Head pose in playspace (the midpoint of the eyes' poses).
    pub head: Pose,
    pub eyes: [EyeInput; 2],
}

impl<G: xr::Graphics> FrameLoop<G> {
    pub fn new(
        session: xr::Session<G>,
        waiter: xr::FrameWaiter,
        stream: xr::FrameStream<G>,
        blend_mode: xr::EnvironmentBlendMode,
    ) -> Result<Self> {
        // STAGE: floor-level origin, what room-scale needs. Quest provides it; fall back to LOCAL
        // with a floor offset in the rig if a runtime lacks it (see `Rig`).
        let space =
            session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;
        Ok(Self {
            session,
            waiter,
            stream,
            space,
            blend_mode,
            running: false,
            focused: false,
            events: xr::EventDataBuffer::new(),
        })
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Drain runtime events and drive the session state machine.
    pub fn pump_events(&mut self, instance: &xr::Instance) -> Result<SessionEvent> {
        let mut out = SessionEvent::None;
        while let Some(event) = instance.poll_event(&mut self.events)? {
            match event {
                xr::Event::SessionStateChanged(e) => match e.state() {
                    xr::SessionState::READY => {
                        self.session.begin(VIEW_TYPE)?;
                        self.running = true;
                    }
                    xr::SessionState::STOPPING => {
                        self.session.end()?;
                        self.running = false;
                    }
                    xr::SessionState::FOCUSED => {
                        self.focused = true;
                        out = SessionEvent::Focused;
                    }
                    xr::SessionState::VISIBLE | xr::SessionState::SYNCHRONIZED => {
                        if self.focused {
                            self.focused = false;
                            out = SessionEvent::Unfocused;
                        }
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        return Ok(SessionEvent::Exit);
                    }
                    _ => {}
                },
                xr::Event::InstanceLossPending(_) => return Ok(SessionEvent::Exit),
                _ => {}
            }
        }
        Ok(out)
    }

    /// Wait for the next frame, begin it, and locate both eyes. Returns `None` while the session
    /// isn't running. The caller renders when `timing.should_render`, then calls [`Self::end`].
    pub fn begin(&mut self) -> Result<Option<FrameState>> {
        if !self.running {
            return Ok(None);
        }
        let state = self.waiter.wait()?;
        self.stream.begin()?;
        let timing = FrameTiming {
            predicted_display_time: state.predicted_display_time,
            predicted_period: state.predicted_display_period,
            should_render: state.should_render,
        };
        if !state.should_render {
            return Ok(Some(FrameState {
                timing,
                head: Pose::IDENTITY,
                eyes: [EyeInput {
                    local: Pose::IDENTITY,
                    fov: qrustcraft_vr::projection::Fov::default(),
                }; 2],
            }));
        }
        let (_, views) =
            self.session
                .locate_views(VIEW_TYPE, state.predicted_display_time, &self.space)?;
        let eye = |v: &xr::View| EyeInput {
            local: convert::pose(v.pose),
            fov: convert::fov(v.fov),
        };
        let eyes = [eye(&views[0]), eye(&views[1])];
        let head = Pose::new(
            (eyes[0].local.position + eyes[1].local.position) * 0.5,
            eyes[0]
                .local
                .orientation
                .slerp(eyes[1].local.orientation, 0.5),
        );
        Ok(Some(FrameState { timing, head, eyes }))
    }

    /// Submit. `layer_views` is `None` when the frame wasn't rendered (an empty frame still has to
    /// be ended).
    pub fn end(
        &mut self,
        time: xr::Time,
        layers: Option<&[xr::CompositionLayerProjectionView<'_, G>; 2]>,
    ) -> Result<()> {
        match layers {
            Some(v) => {
                let layer = xr::CompositionLayerProjection::new()
                    .space(&self.space)
                    .views(v);
                self.stream.end(time, self.blend_mode, &[&layer])?;
            }
            None => self.stream.end(time, self.blend_mode, &[])?,
        }
        Ok(())
    }

    pub fn stream_mut(&mut self) -> &mut xr::FrameStream<G> {
        &mut self.stream
    }
}
