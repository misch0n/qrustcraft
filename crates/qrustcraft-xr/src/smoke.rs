//! Headset smoke test, independent of Minecraft: if this works, the XR stack (loader, runtime,
//! Vulkan device negotiation, wgpu bridge, swapchain, controllers) works on your machine.
//!
//! Each eye is cleared to a different colour; the colour brightens with the right trigger, and the
//! right controller buzzes when you pull it. Run with a headset connected (Quest Link / Air Link /
//! Virtual Desktop / SteamVR / Monado):
//!
//! ```text
//! cargo run -p qrustcraft-xr --features wgpu-bridge --example stereo_smoke
//! ```

use crate::{
    Controllers, FrameLoop, SessionEvent, XrContext,
    vulkan::{EyeSwapchain, XrVulkan},
};
use openxr as xr;
use qrustcraft_vr::{
    Hand,
    haptics::Pulse,
    input::{Bindings, IntentMapper},
    projection::Depth,
    rig::Rig,
};

/// Run until the session ends or `quit()` returns true.
pub fn run(
    ctx: XrContext,
    mut quit: impl FnMut() -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!(
        "runtime: {} | eye {}x{} (max {}x{})",
        ctx.runtime,
        ctx.eye_size.0,
        ctx.eye_size.1,
        ctx.max_eye_size.0,
        ctx.max_eye_size.1
    );

    let vk = XrVulkan::create(&ctx, &wgpu::DeviceDescriptor::default())?;
    log::info!("gpu: {}", vk.adapter.get_info().name);
    let (session, waiter, stream) = vk.create_session(&ctx)?;
    ctx.request_refresh_rate(&session);
    let controllers = Controllers::new(&ctx.instance, &session)?;
    let mut swapchain = EyeSwapchain::new(&vk, &session, ctx.eye_size)?;
    let mut frames = FrameLoop::new(session, waiter, stream, ctx.blend_mode)?;

    let (rig, mut mapper, bindings) =
        (Rig::default(), IntentMapper::default(), Bindings::default());
    let mut last_trigger = false;
    'main: loop {
        if quit() {
            break 'main;
        }
        match frames.pump_events(&ctx.instance)? {
            SessionEvent::Exit => break 'main,
            SessionEvent::Unfocused => log::info!("headset off / unfocused"),
            _ => {}
        }
        let Some(frame) = frames.begin()? else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        };
        let time = frame.timing.predicted_display_time;
        if !frame.timing.should_render {
            frames.end(time, None)?;
            continue;
        }

        let (pads, poses) = controllers.poll(&frames.session, &frames.space, time)?;
        let intent = mapper.update(&bindings, &pads[0], &pads[1], 1.0 / 72.0, false);
        if intent.use_item_pressed && !last_trigger {
            controllers.vibrate(
                &frames.session,
                Pulse {
                    hand: Hand::Right,
                    amplitude: 0.6,
                    duration_ms: 60,
                },
            )?;
        }
        last_trigger = intent.use_item;

        // The same call the game makes: playspace -> world views.
        let world = rig.frame(
            frame.head,
            frame.eyes,
            poses.grip,
            qrustcraft_vr::DVec3::ZERO,
            0.05,
            512.0,
            Depth::ReversedZeroToOne,
        );
        let _ = world.eyes[0].view; // a game would render with these; the smoke test just clears.

        let image = swapchain.acquire()?;
        let mut encoder = vk.device.create_command_encoder(&Default::default());
        for eye in 0..2 {
            let view = swapchain.view(image, eye);
            let glow = f64::from(pads[1].trigger) * 0.6;
            let colour = if eye == 0 {
                wgpu::Color {
                    r: 0.1 + glow,
                    g: 0.05,
                    b: 0.3,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: 0.3,
                    g: 0.05,
                    b: 0.1 + glow,
                    a: 1.0,
                }
            };
            drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(colour),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            }));
        }
        vk.queue.submit([encoder.finish()]);
        swapchain.release()?;

        let to_xr = |e: usize| {
            let (pose, fov) = (frames_pose(&frame, e), frames_fov(&frame, e));
            swapchain.layer_view(e, pose, fov)
        };
        let views = [to_xr(0), to_xr(1)];
        frames.end(time, Some(&views))?;
    }
    Ok(())
}

// The loop returns playspace eyes in qrustcraft types; the compositor wants the raw OpenXR ones back.
fn frames_pose(f: &crate::frame::FrameState, e: usize) -> xr::Posef {
    let p = f.eyes[e].local;
    xr::Posef {
        orientation: xr::Quaternionf {
            x: p.orientation.x,
            y: p.orientation.y,
            z: p.orientation.z,
            w: p.orientation.w,
        },
        position: xr::Vector3f {
            x: p.position.x,
            y: p.position.y,
            z: p.position.z,
        },
    }
}

fn frames_fov(f: &crate::frame::FrameState, e: usize) -> xr::Fovf {
    let v = f.eyes[e].fov;
    xr::Fovf {
        angle_left: v.left,
        angle_right: v.right,
        angle_up: v.up,
        angle_down: v.down,
    }
}
