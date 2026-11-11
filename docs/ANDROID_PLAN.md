# Path to a sideloadable Quest APK

Goal: `qrustcraft-quest.apk`, installed with SideQuest (or `adb install`), running Horizon in VR on
a Quest 3 with no PC attached.

## Stages

| Stage | Result | State |
|---|---|---|
| 1. Smoke APK | An APK that opens an OpenXR session on the Quest, clears each eye to a colour, and buzzes a controller on trigger. Proves OpenXR + Vulkan + wgpu + Touch input on the device, with no Minecraft code | **Built and signed** (`target/qrustcraft-quest.apk`, inspected: arm64, correct entry point, loader bundled); **not yet run on a headset** |
| 2. Horizon on Android | Horizon's game loop runs in a NativeActivity with its own files | Type-checks for Android with 4 small patches; entry point, full link and device run still to do |
| 3. VR integration | Stereo views, hands, input, GUI panel (`INTEGRATION.md`) | Designed; core logic done and tested |
| 4. Quest performance | Playable frame rate on Adreno (Quest 3) | Not started |

Stage 1 is deliberately first: it is small, it needs none of Horizon, and it answers the biggest
unknown (does the XR stack work on the device) cheaply. Everything after it builds on a proven base.

## Stage 2: what stands between Horizon and Android

Measured, not guessed: Horizon at `5070174` was type-checked for `aarch64-linux-android` with the
NDK (`cargo ndk -t arm64-v8a check -p horizon-client --lib --no-default-features --features
audio,horizon-extras`). It failed four times, each fixed by a small patch
(`patches/horizon/0001-*.patch`), and then **passed**:

| # | Failure | Cause | Fix |
|---|---|---|---|
| 1 | `android-activity`: "Either game-activity or native-activity must be enabled" | winit's Android backend needs one activity implementation | `winit` with `android-native-activity` on `target_os = "android"` (no Java needed) |
| 2 | `arboard`: `Clipboard` not found | the clipboard crate has no Android backend | gate it out; Android's `SystemClipboard` keeps text in the game (the same fallback Horizon already has when no system clipboard is reachable) |
| 3 | `winit::platform::modifier_supplement` missing | `key_without_modifiers` is desktop-only; used to learn keyboard layouts | `learn_layout` is a no-op on Android |
| 4 | `cem.rs`: `own.insert` on a `Vec` | **an upstream bug**: a local `own` shadows the outer map. Fails on desktop too | rename the local (worth reporting upstream) |

Also: `hidapi` (DualSense) fails its build script on Android, so the `controllers` feature is left
off, which a VR build wants anyway (controllers come from OpenXR). The 262k-line renderer
(`horizon-client-render`) compiled for Android with no changes.

Still to do for stage 2 (type-checking is not linking or running):

1. **Entry point.** Horizon's `main()` is a desktop executable. Android needs `android_main`
   (`qrustcraft-quest`) that stores the `AndroidApp`, sets the working directory to the app's files
   folder, and calls `entry::main_with`. Horizon's three `EventLoop::new()` call sites must use
   `with_android_app(app)` on Android (winit panics otherwise).
2. **Full compile and link.** A real `cargo ndk build` (codegen plus C dependencies such as `ring`
   and `mimalloc`) of the whole game: in progress.
3. **No Java on a Quest.** `horizon-prepare` runs Mojang's exporters through a JDK. Plan: prepare on
   your PC with Horizon's launcher or `horizon-prepare` (using *your* copy of Minecraft), then
   `tools/push-game-files.sh` copies the result into the app's folder over adb. Mojang's files are never
   inside the APK. (Horizon issue #1750 is working toward Java-free preparation.)
4. **Paths.** Saves, options and `.local/horizon-client/26.3` are relative to the working directory;
   pointing the working directory at `/sdcard/Android/data/dev.qrustcraft.quest/files/` covers them.
5. **Audio** (`cpal` 0.16 supports Android via Oboe), and **threads/memory** (Horizon issue #1751): needs
   a run on the device.
6. **Rendering**: Stage 3's per-view passes and the asymmetric-FOV shader fixes.

## What was blocked, and what you approved

- The Android NDK/SDK come from `dl.google.com`, and the OpenXR loader from Maven Central. You allowed
  both hosts for the sandbox; with them the stage 1 APK builds here (`tools/build-apk.sh`).
- Building Horizon runs its build scripts (third-party code). You approved that for this sandbox.

## Risks, stated plainly

- Stage 2 is a real porting project in a 1.7M-line codebase that has never been built for Android.
  Expect surprises past the ones found by reading.
- Nothing here has run on a headset. Stage 1 exists to find out early.
- Quest performance of a full Minecraft world is unproven; Horizon's benchmarks are desktop.
- Horizon's Android issues are open upstream; coordinating with them (and its layering rules) will
  save rework.
