//! Horizon as an Android NativeActivity.
//!
//! Stage 2a: the unmodified game, started the Android way. On a Quest this opens as a 2D panel
//! (no immersive OpenXR session yet); it proves Horizon runs on the headset's Android, with its
//! game files pushed from a PC (`tools/push-game-files.sh`). Stage 3 replaces the flat window
//! with stereo views driven by `qrustcraft-xr`.

#![cfg(target_os = "android")]

use winit::platform::android::activity::AndroidApp;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("qrustcraft"),
    );
    log::info!("starting Horizon");
    horizon_client::entry::run_android(app);
}
