# TODO for the next session

State at the end of the last session (branch `claude/lucid-babbage-3w7esu`):

- `qrustcraft-vr`: 87 tests passing (rig, projection, input, posture, bow, climb, swing, teleport,
  GUI panel, pose protocol). A review pass fixed: climbing in room axes instead of world axes, f32
  rig origin (head jitter far from spawn), swings from tracking jumps, f32 gesture clock, teleports
  onto wall sides, the rig not following the body vertically, sneak flicker, GUI cursor edges,
  untracked hands in the pose packet; and in the XR layer, refresh-rate requests, swapchain format
  negotiation, API level from the device, no panics on short runtime lists.
- Two APKs built and signed, **neither run on a headset yet**:
  - `qrustcraft-smoke.apk`: OpenXR + Vulkan + wgpu + Touch input test. Flat colours, no Minecraft.
  - `qrustcraft-horizon.apk`: Horizon as an Android app (a flat panel, not VR). 35 MB.
- Horizon pinned at `third_party/horizon.pin`, patched by `patches/horizon/0001`, `0002`.

## 0. Rebuild the sandbox first (the container is ephemeral)

None of this survives a new session. The network allowlist (dl.google.com, repo1.maven.org) is
saved on the environment, so the downloads will work.

- [ ] `rustup target add aarch64-linux-android` and `cargo install cargo-ndk --locked`
- [ ] Android command-line tools into `/home/user/android-sdk`, then
      `sdkmanager "build-tools;34.0.0" "platforms;android-34" "platform-tools" "ndk;26.3.11579264"`
      (needs the SDK licence accepted)
- [ ] OpenXR loader: `org.khronos.openxr:openxr_loader_for_android:1.1.65` AAR from Maven Central,
      `jni/arm64-v8a/libopenxr_loader.so`. Maven Central rate-limits shared IPs; retry with backoff.
- [ ] `tools/setup-horizon.sh` (fetches the pin, applies the patches; ~190 MB)
- [ ] Consider saving these steps as the environment's setup script so later sessions start ready.

## 1. Results from the headset (needs the user)

- [ ] Smoke APK on a Quest 3: does each eye show its colour, does the right trigger brighten it and
      buzz? If not, get `adb logcat -s qrustcraft`.
- [ ] Horizon APK: does it reach Horizon's "missing resources" screen? Then push prepared game files
      (`tools/push-game-files.sh`, untested) and see whether the title screen and a world load.
- [ ] Fix whatever these turn up (likely: Vulkan driver quirks on Adreno, audio, file paths,
      thread limits, memory).

## 2. VR inside Horizon (stage 3, see `docs/INTEGRATION.md`)

- [ ] One APK: the Horizon app opens an OpenXR session (`qrustcraft-xr`) instead of a flat window.
      Needs `qrustcraft-xr` in `app/`, the immersive manifest (VR intent category, OpenXR loader).
- [ ] Device from OpenXR: Horizon opens its wgpu device through `XrVulkan::create` when VR is on.
- [ ] `ViewSet::stereo([View; 2], cull)` in `gfx/view.rs`; draw the world pass per view into the
      swapchain's array layers. TAA and frame generation off in VR at first.
- [ ] Camera from the head (`Rig::frame` with Horizon's `camera: DVec3` as the camera origin,
      `head_yaw_pitch`); room-scale walking via `Rig::absorb_walk`, fed every entity move.
- [ ] Held item at the grip pose; aim and swing from `interact` (`SwingTracker`, `ToolClass::reach`).
- [ ] `input::Intent` into the gamepad action path; posture (sneak/crawl/jump), bow, climbing.
- [ ] GUI panel: render screens to a texture, draw on the quad, ray cursor clicks real widgets.
- [ ] Asymmetric FOV: grep `tan_half_fov` in Horizon shaders, use the off-axis projection.

## 3. Quest performance (stage 4)

- [ ] Default render distance and LOD for Quest; resolution scale.
- [ ] Fixed foveated rendering and the hidden-area mesh.
- [ ] Measure frame time on device at 72/90 Hz.

## 4. Later rows of `docs/VIVECRAFT_STEPS.md`

- [ ] Mirror window, seated mode, eating/drinking, backpack, swimming strokes, rowing, full-body tracking.
- [ ] Wire `pose_net` into Horizon's protocol (capability handshake, server hit validation) and
      render other players' hands.

## Housekeeping

- [ ] Report the shadowed-variable bug in Horizon's `mobs/cem.rs` upstream (fixed in our 0001).
      Commits and pushes stay in our repo only; an upstream report needs the user's go-ahead.
- [ ] Smoke APK manifest asks for `RECORD_AUDIO`, which marks the microphone as required; drop it
      if unused.
- [ ] CI for this repo: `cargo test`, the Android type-check, the APK build.
