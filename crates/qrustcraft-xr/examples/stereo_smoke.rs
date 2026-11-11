//! Headset smoke test on a desktop runtime (Quest Link / Air Link / Virtual Desktop / SteamVR / Monado).
//!
//! ```text
//! cargo run -p qrustcraft-xr --features wgpu-bridge --example stereo_smoke
//! ```
//! The same loop runs on the headset itself as the `qrustcraft-quest` APK.

use qrustcraft_xr::{XrContext, XrOptions, smoke};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    smoke::run(XrContext::new(&XrOptions::default())?, || false)
}
