# Path to a sideloadable Quest APK

Goal: `qrustcraft-quest.apk`, installed with SideQuest (or `adb install`), running Horizon in VR on
a Quest 3 with no PC attached.

## Stages

| Stage | Result | State |
|---|---|---|
| 1. Smoke APK | An APK that opens an OpenXR session on the Quest, clears each eye to a colour, and buzzes a controller on trigger. Proves OpenXR + Vulkan + wgpu + Touch input on the device, with no Minecraft code | Code written; type-checks for `aarch64-linux-android`; **not linked, packaged or run** (see "What is missing") |
| 2. Horizon on Android | Horizon's game loop runs in a NativeActivity with its own files | Not started; blockers listed below |
| 3. VR integration | Stereo views, hands, input, GUI panel (`INTEGRATION.md`) | Designed; core logic done and tested |
| 4. Quest performance | Playable frame rate on Adreno (Quest 3) | Not started |

Stage 1 is deliberately first: it is small, it needs none of Horizon, and it answers the biggest
unknown (does the XR stack work on the device) cheaply. Everything after it builds on a proven base.

## Stage 2: what stands between Horizon and Android

From a static read of Horizon's manifests and source at `5070174` (nothing was built):

1. **No Android entry point.** There is no `android_main`, no `android-activity`, and winit is used
   without its Android backend feature. Horizon needs a NativeActivity entry that creates its
   window/event loop from an `AndroidApp`. (Horizon already has Android input *state* modules,
   `android_runtime.rs` and `android_gamepad.rs`, adapted from the Starling project, but nothing
   feeds them yet.)
2. **Desktop-only dependencies that need `cfg` gating**: `arboard` (clipboard, in
   `horizon-client-input`), `hidapi` (in `horizon-dualsense`), `gilrs` (controllers), `open`. Each has
   to be compiled out on Android; the first two have no Android backend.
3. **No Java on a Quest.** `horizon-prepare` runs Mojang's own exporters through a JDK. Plan: do the
   preparation on your PC with Horizon's launcher or `horizon-prepare` (using *your* copy of Minecraft),
   then `tools/push-game-files.sh` copies the result to the app's private folder over adb. Mojang's
   files are never inside the APK. (Horizon issue #1750 is working toward Java-free preparation, which
   would let the headset do this itself.)
4. **Paths.** Horizon keeps saves and options in the working directory and reads prepared files from
   `.local/horizon-client/26.3`. On Android both must point into
   `/sdcard/Android/data/dev.qrustcraft.quest/files/`.
5. **Audio** is `cpal` 0.16, which supports Android (Oboe), but needs a build check and a
   permission/latency pass.
6. **Native code** (`ring` via rustls, `mimalloc`) builds with the NDK's clang; expected fine, to be
   confirmed on a real build.
7. **Helper-thread limits and memory.** Horizon issue #1751 covers thread limits on Android. A Quest 3
   has 8 GB shared with the OS; Horizon's working set (1.1-1.7 GB measured on desktop) fits, but render
   distance and LOD must default low.
8. **Rendering.** wgpu's Vulkan backend works on Quest (Adreno). Stage 3's per-view passes and the
   asymmetric-FOV shader fixes are the main renderer work.

## What is missing here, concretely

This sandbox cannot produce the APK, for two reasons that are decisions for you:

- **The Android NDK and SDK build-tools are not reachable.** They download from `dl.google.com`,
  which the sandbox's network policy blocks. Without the NDK there is no linker for the
  `aarch64-linux-android` target. (The Rust target itself is installed and the crate type-checks.)
- **Building Horizon means executing its code.** A Horizon build runs its `build.rs` scripts and
  procedural macros, which is third-party code. That was blocked until you approve it.

Neither blocks the work you can do on your own PC: `tools/build-apk.sh` is written for exactly this
and needs only the NDK, SDK build-tools, `cargo-ndk` and the Khronos OpenXR loader.

## Risks, stated plainly

- Stage 2 is a real porting project in a 1.7M-line codebase that has never been built for Android.
  Expect surprises past the ones found by reading.
- Nothing here has run on a headset. Stage 1 exists to find out early.
- Quest performance of a full Minecraft world is unproven; Horizon's benchmarks are desktop.
- Horizon's Android issues are open upstream; coordinating with them (and its layering rules) will
  save rework.
