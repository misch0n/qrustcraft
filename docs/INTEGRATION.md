# Integrating into Horizon

Pinned reference: Horizon at `5070174` (`third_party/horizon.pin`) (2026-11-10). Paths are relative to
`crates/horizon-client-render/src/` unless noted.

## The seam already exists

`gfx/view.rs` says: *"A flat screen has one [view]; a headset has one per eye. Every view shares the
frame's camera position, so the camera-relative world is built once and only each view's rotation
and lens differ."* Its `View` is `{ view, projection (reversed-Z), aspect, tan_half_fov }` and
`ViewSet` holds up to `MAX_VIEWS = 2`. `Renderer::views(frame)` (gfx.rs ~7783) returns
`ViewSet::single(..)` today. That is exactly `qrustcraft_vr::EyeView`:

| Horizon `View` | `qrustcraft_vr::EyeView` |
|---|---|
| `view` (camera-relative, includes the eye offset) | `view` from `Rig::frame(.., camera_origin, ..)` |
| `projection` (reversed-Z, zero-to-one) | `projection(fov, near, far, Depth::ReversedZeroToOne)` |
| `aspect` | `EyeView::aspect()` |
| `tan_half_fov` | `EyeView::tan_half_fov_y()`, but see asymmetric FOV below |

## Work items, in order

1. **`ViewSet::stereo([View; 2], cull)`.** `cull` must be a view whose frustum covers both eyes
   (widest FOV, eyes' midpoint). `primary()` stays the left eye for the HUD.
2. **Draw per view.** `Renderer::render` (gfx.rs ~7959) computes `view_proj` once from
   `views.primary()` (~8305). Loop the world pass over `views.views()`, target the matching array
   layer of the XR swapchain, and keep the camera-relative section/entity setup outside the loop.
   Passes that read the previous frame (TAA, frame generation, `reproject`) must be disabled or made
   per-eye first; start with TAA and frame generation off in VR.
3. **Device from OpenXR.** Where the game opens its wgpu device, call `XrVulkan::create(&ctx, &desc)`
   instead of `request_device` when VR is on (Horizon's own precedent: `horizon-hdr::open_vulkan_device`
   uses `open_with_callback`). The window surface is not used in VR; Horizon's `compositor.rs`
   already treats the surface as one output among several.
4. **Camera from the head.** Each frame: `Rig::frame(...)` gives eye poses; `head_yaw_pitch` gives
   `Frame::yaw/pitch` for gameplay (`camera.rs`: `Camera`, `CameraEntity`). Feed
   `Rig::absorb_walk` the entity's post-collision delta so room-scale walking respects walls.
5. **Hands.** The first-person hand (`hand_view_proj`, gfx.rs ~8521) becomes the held item at the
   grip pose. Aim rays come from `interact::aim_ray`; swing detection decides attacks.
6. **Input.** Map `input::Intent` onto the same actions the gamepad path feeds
   (`horizon-client-input` `gamepad.rs`, `crates/horizon-client-render/src/android_gamepad.rs`).
   Horizon's own rule: controls must be drawn with a *registering* helper (`ui::button`) to be
   reachable by controller; `ui::GuiPanel`'s cursor then clicks real widgets.
7. **GUI panel.** Render the 2D screen to a texture (the HUD pass already composes to a target),
   draw it on the panel quad each eye.

## Known hazards

- **Asymmetric FOV.** Quest lenses are asymmetric. Shaders that rebuild view rays from
  `tan_half_fov` and `aspect` alone (sky, fog, post) will be off-centre. Grep for `tan_half_fov`
  and use the full off-axis projection (`EyeView::asymmetry()`) there. Do this before judging
  image quality.
- **Single-view assumptions.** Anything cached per frame in screen space (occlusion culling,
  `gpu_occlusion.wgsl`, `cull`) must use the combined cull view.
- **Frame budget.** Quest 3 is 72-120 Hz at about 2064x2208 per eye; Horizon's benchmarks are on a
  desktop GPU. Expect to rely on render distance, `lod`, and fixed foveated rendering on standalone.
- **Layers rule.** Horizon's `tools/check_layers.py` forbids Horizon-layer code reaching into the
  engine. Put the VR glue in the engine/platform layer, or maintain it as a patch series.

## Why a library and a patch series, not a fork of Horizon

Horizon moves fast (hundreds of commits a month on the renderer). A long-lived fork would drift;
keeping the VR logic here and the glue small keeps rebasing cheap. The glue is expected to be a
few hundred lines in Horizon.
