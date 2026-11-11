//! OpenXR layer for QRustCraft.
//!
//! [`XrContext`] owns the OpenXR instance and system and tells you what the headset wants.
//! [`Controllers`] is the Touch (and compatible) action set. [`FrameLoop`] runs wait/begin/end
//! and hands the game a [`qrustcraft_vr::rig::EyeInput`] pair per frame. Graphics binding lives in
//! [`vulkan`] (behind `wgpu-bridge`), because OpenXR must create the Vulkan device itself.

pub mod actions;
pub mod context;
pub mod convert;
pub mod frame;
#[cfg(feature = "wgpu-bridge")]
pub mod vulkan;

pub use actions::Controllers;
pub use context::{XrContext, XrOptions};
pub use frame::{FrameLoop, FrameState, FrameTiming, SessionEvent};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OpenXR: {0}")]
    Xr(#[from] openxr::sys::Result),
    #[error("could not load the OpenXR loader: {0}")]
    Loader(String),
    #[error("the OpenXR runtime lacks a required extension: {0}")]
    MissingExtension(&'static str),
    #[error("graphics: {0}")]
    Graphics(String),
}

pub type Result<T> = std::result::Result<T, Error>;
