//! QRustCraft for Meta Quest: the `NativeActivity` entry point.
//!
//! Stage 1 (this file): run the headset smoke test on the device, so the first APK proves the
//! hard part (OpenXR session, Vulkan device negotiation, wgpu swapchain, Touch input and haptics
//! on a real Quest) before any Minecraft code is involved. Stage 2 replaces `smoke::run` with
//! Horizon's game loop driven through the same `XrContext` (see docs/ANDROID_PLAN.md).

#![cfg(target_os = "android")]

use android_activity::{AndroidApp, MainEvent, PollEvent};
use qrustcraft_xr::{XrContext, XrOptions, smoke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Quest 2/3 run Android 12L (API 32).
const QUEST_SDK: u32 = 32;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("qrustcraft"),
    );
    log::info!("QRustCraft starting");

    let opts = XrOptions {
        android_sdk_version: QUEST_SDK,
        ..XrOptions::default()
    };
    let ctx = match XrContext::new(&opts, app.vm_as_ptr(), app.activity_as_ptr()) {
        Ok(c) => c,
        Err(e) => {
            log::error!("OpenXR init failed: {e}");
            return;
        }
    };
    log::info!(
        "runtime {} eye {}x{}",
        ctx.runtime,
        ctx.eye_size.0,
        ctx.eye_size.1
    );

    let quit = AtomicBool::new(false);
    let result = smoke::run(ctx, || {
        app.poll_events(Some(Duration::ZERO), |event| {
            if let PollEvent::Main(MainEvent::Destroy) = event {
                quit.store(true, Ordering::Relaxed);
            }
        });
        quit.load(Ordering::Relaxed)
    });
    if let Err(e) = result {
        log::error!("smoke test failed: {e}");
    }
}
