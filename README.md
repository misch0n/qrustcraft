# QRustCraft

Minecraft in VR, without Java: a VR layer for [Horizon](https://github.com/horizon-rust/horizon)
(Minecraft: Java Edition rewritten in Rust on wgpu), aimed at the Meta Quest 3.

It does what [QuestCraft](https://github.com/QuestCraftPlusPlus/QuestCraft) does for the player
(room-scale Minecraft on a Quest, Vivecraft-style controls) by a shorter road. QuestCraft runs the
Java game on Android and translates its OpenGL calls; here the game is already native Rust on
Vulkan, and OpenXR plugs straight into it. See [docs/DESIGN.md](docs/DESIGN.md).

The design starts from Vivecraft's own steps: [docs/VIVECRAFT_STEPS.md](docs/VIVECRAFT_STEPS.md)
lists each thing Vivecraft had to do to make Minecraft VR, its weaknesses, and what this project
does instead, with a status per step.

## Status: foundation, not yet playable

| Piece | State |
|---|---|
| `qrustcraft-vr`: stereo projection (asymmetric, reversed-Z), room-scale rig, snap/smooth turn, locomotion, teleport arc, swing-to-mine and per-tool reach, bow, climbing, physical sneak/crawl/jump, floating GUI panel, haptics, comfort vignette, pose network format with server hit validation | Done, 71 unit tests |
| `qrustcraft-xr`: OpenXR instance, Touch actions, frame loop | Compiles; **not yet run on a headset** |
| `qrustcraft-xr` `wgpu-bridge`: OpenXR-created Vulkan device and swapchain images exposed to wgpu 30 | Compiles against wgpu 30.0.1; **not yet run on a headset** |
| `stereo_smoke` example: end-to-end headset check with no Minecraft | Compiles; **not yet run on a headset** |
| `qrustcraft-quest`: NativeActivity app that runs the smoke test on the headset (stage 1 APK) | **APK built and signed**, inspected; **not yet run on a headset** |
| APK packaging: manifest, `tools/build-apk.sh` | Works (produced the APK); `tools/push-game-files.sh` untested |
| Horizon integration (stereo `ViewSet`, VR input, GUI panel) | Designed, see [docs/INTEGRATION.md](docs/INTEGRATION.md); not started |
| Horizon on Android | Game library **type-checks for Android** with 4 small patches in `patches/horizon/`; entry point, full link and device run still to do. See [docs/ANDROID_PLAN.md](docs/ANDROID_PLAN.md) |

Nothing here has touched real XR hardware yet. Run `stereo_smoke` first; it is the cheapest way to
find out whether the headset stack works on your machine.

## Try it

```sh
cargo test --workspace
# With a headset connected (Quest Link / Air Link / Virtual Desktop / SteamVR / Monado):
cargo run -p qrustcraft-xr --features wgpu-bridge --example stereo_smoke
```

## Roadmap

1. **PCVR on Quest 3** (Link, Air Link, Virtual Desktop, ALVR) via desktop OpenXR. Horizon already
   builds for Windows and Linux, so this needs no Android port and is playable on a Quest 3 today.
2. **Quest standalone** (the goal: a SideQuest-installable APK, see [docs/ANDROID_PLAN.md](docs/ANDROID_PLAN.md) and [docs/SIDELOAD.md](docs/SIDELOAD.md)): an Android build of Horizon (`aarch64-linux-android`, Vulkan, Meta's OpenXR
   loader) packaged as an APK. Horizon lists Android as a long-term goal with contributors on the
   first pieces; its game files must be prepared without Java first (Horizon issue #1750).

## Layout

- `crates/qrustcraft-vr`: engine-agnostic VR core, no GPU or OpenXR dependency.
- `crates/qrustcraft-xr`: OpenXR runtime layer and the Vulkan-to-wgpu bridge.
- `crates/qrustcraft-quest`: the Android/Quest app (NativeActivity). `android/` has its manifest; `tools/` the build and file-push scripts.
- `third_party/horizon`: Horizon, pinned as a submodule (`git submodule update --init --depth 1`).
- `docs/`: design and integration notes.

## Licence and credits

MIT. No Minecraft code or assets are included; you need your own copy of Java Edition, as Horizon
requires. The control scheme (hand-aimed tools, swing to mine, room-scale rig) follows ideas
proven in [Vivecraft](https://github.com/Vivecraft/VivecraftMod) (LGPLv3); this code is written
from those ideas and copies none of Vivecraft's source.
