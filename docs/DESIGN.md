# Design: the same result as QuestCraft, by a shorter road

## What QuestCraft has to do

From its README: a launcher app (Pojlib, derived from PojavLauncher) runs **Java Edition inside
Android** with Fabric and the Vivecraft mod, translates the game's OpenGL calls to the headset's
graphics API with a "Light Thin Wrapper", and bridges OpenXR through a wrapper library. Because
the game is a JVM program that was never meant for a phone-class GPU, it carries a launcher,
version-specific mod sets, and a performance guide (render distance 4-6, resolution at 80%).

## What changes with a native Rust game

| QuestCraft step | Why it exists | Here |
|---|---|---|
| JVM on Android | Minecraft is Java | Not needed: Horizon has no Java |
| OpenGL to GLES/Vulkan wrapper | The game speaks OpenGL | Not needed: Horizon speaks Vulkan through wgpu |
| OpenXR wrapper into the JVM | Java can't call OpenXR directly | A Rust crate (`openxr`) calls it directly |
| Vivecraft mod patching Minecraft internals per version | The game must be modified in place | A library the game calls through one seam (`ViewSet`) |
| Per-version mod sets, Fabric | Each Minecraft version differs | Horizon pins one version (26.3) |

The remaining hard part is real and is the same one QuestCraft solved: **fitting the game's
rendering and input to a headset**. That is what this repository does.

## Principles

1. **Engine-agnostic core.** `qrustcraft-vr` has no GPU, OS or OpenXR dependency, so all VR logic
   (rig, locomotion, interaction, input mapping) is unit-tested on any machine and can be driven by
   a simulator. Bugs in turning or reach are found by `cargo test`, not by putting a headset on.
2. **OpenXR owns the Vulkan device.** OpenXR must create the Vulkan instance and device so the
   runtime can add its extensions and choose the headset's GPU. wgpu normally does both. We ask
   wgpu-hal which extensions and features the game's `wgpu::Features` need, let OpenXR create the
   objects with exactly those, and hand them back via `from_raw` / `device_from_raw`
   (`qrustcraft-xr/src/vulkan.rs`). The same pattern Horizon already uses for HDR
   (`horizon-hdr::open_vulkan_device`).
3. **Hand-aimed, physically swung tools.** As in Vivecraft: tools act where the hand points,
   attacks need a real swing (`interact::SwingDetector`), mining needs sustained motion on a block
   (`MiningHold`), and the player entity follows your physical walking while collision still applies
   (`Rig::absorb_walk`).
4. **Comfort is not optional.** Snap turn by default, turns pivot about the head so they cannot
   shift you sideways, and a vignette that reacts only to *artificial* motion (`comfort`).
5. **Menus are a panel, not a mod.** The 2D GUI is drawn to a texture shown on a quad; a
   controller ray becomes the cursor (`ui::GuiPanel`). Horizon already registers widgets for
   controllers, so no screen needs rewriting.

## Coordinates

OpenXR: +X right, +Y up, -Z forward. Minecraft: +X east, +Y up, +Z south; yaw 0 faces +Z and grows
clockwise from above. `coords::rig_rotation(yaw)` is the one place that conversion lives, with
tests for each axis.
