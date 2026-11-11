# Vivecraft's steps, and what we do with each

Source: a read of `Vivecraft/VivecraftMod` (target Minecraft 26.2; 582 Java files, 149 common
mixins plus 55 compat mixins). Each row is a step Vivecraft had to execute to make Java Minecraft
playable in VR, the weakness that step carries, and what this project does.

**Status** is one of: *Done* (code and tests in this repo), *Designed* (the approach is settled
and documented, no code yet), *Later*. Nothing has run on a headset yet.

Why the table can improve on Vivecraft at all: Vivecraft is a mod grafted onto an engine that
assumed one camera, one framebuffer, a mouse and a point-sized player. Horizon is a clean engine we
can add VR to directly, so most of Vivecraft's cost (mixins that break each release, faked input,
a full frame re-run per eye) simply doesn't arise.

## A. Rendering

| # | Vivecraft step | Weakness | Our version | Status |
|---|---|---|---|---|
| A1 | Hook the frame loop, poll the VR runtime, replace the window present | Global mutable "main render target" swapped per pass; many mixins branch on pass type | `FrameLoop` owns wait/begin/end; the renderer is told a `ViewSet`, no global state | Done (`qrustcraft-xr::frame`) |
| A2 | Build per-eye camera from HMD pose and the runtime's head-to-eye transform | Camera set as yaw/pitch plus a model-view hack so billboards stay world-up | `Rig::frame` yields a full camera-relative view matrix per eye; Horizon's `View` already takes a matrix | Done (`rig`) |
| A3 | Per-eye projection from the runtime's raw frustum; near 0.02; reverse-Z | Rebuilt/cached per far plane; extra "undistorted" projection for HUD | `projection()` from `XrFovf`, asymmetric, reverse-Z or forward-Z; `EyeView::asymmetry()` exposes the lens offset | Done |
| A4 | Render-target layout: one shared 3D target reused by both eyes, then post into 2 eye textures | Many large targets; full reinit on setting changes | One 2-layer swapchain image, one render pass per layer, wrapped into wgpu | Done, headset-untested (`vulkan::EyeSwapchain`) |
| A5 | **Run the whole vanilla frame once per eye** (update, extract, render) | The biggest cost: nothing shared between eyes; Iris/OptiFine need hacks | Horizon builds the camera-relative world once; only rotation and lens differ per view (`gfx/view.rs`). Per-eye work is the draw passes only | Designed (`INTEGRATION.md` 1-2) |
| A6 | Cull per eye by invalidating frustum/occlusion before each pass | Re-cull, chunk flicker | One cull view whose frustum covers both eyes (`ViewSet::cull`) | Designed |
| A7 | Rain/snow centred on HMD so eyes agree; sky/clouds/particles per eye | No stereo reuse | Same camera origin for both views, so these are shared by construction | Designed |
| A8 | Draw hands/items as world geometry at the controller pose; separate "hands" target | Many per-item mixins | Held item drawn at the grip pose in the normal world pass; depth-correct for free | Designed (`INTEGRATION.md` 5) |
| A9 | Hidden-area stencil mesh, FSAA, render scale | Stencil path currently disabled upstream; no foveation or variable-rate shading | Render scale first; fixed foveation and the runtime's hidden-area mesh planned for Quest standalone | Later |
| A10 | Submit eye textures to the compositor | OpenGL/Vulkan interop special cases (blocked on Intel+Windows) | Native Vulkan swapchain from OpenXR, no interop copy | Done, headset-untested |
| A11 | Desktop mirror (single eye, centre, mixed reality) | CENTER/THIRD mirrors are extra full world renders | Mirror = blit of an eye image; extra views only when asked | Later |
| A12 | Menu world / menu room (~4k lines duplicating renderer logic) | Maintained per MC version | Horizon's own title scene rendered through the same views | Later |

## B. GUI

| # | Vivecraft step | Weakness | Our version | Status |
|---|---|---|---|---|
| B1 | Draw screens into an off-screen texture with a **faked window size** | Fixed resolution: text blurry past ~1.25 m; mods that ignore the fake size misbehave | Same quad idea, but resolution follows distance and size; panel owns its pixel size (`GuiPanel::gui_px`) | Done (math), rendering Designed |
| B2 | Quad 1.5 units wide, 1 m in front of the HMD, faces it; chat/book offsets; container screens float over the block | Placement rules scattered | `GuiPanel::in_front_of` ignores head pitch so it doesn't tilt with your gaze | Done |
| B3 | Controller ray -> quad intersection -> **fake GLFW mouse events** (with 0.7/0.3 smoothing) | Faking the OS input layer is fragile; smoothing adds latency | Ray hit gives GUI pixels directly; Horizon registers widgets for controllers so there is no OS faking (`ui::Cursor` emits move/press/release) | Done |
| B4 | HUD modes: head-locked, on hand, on wrist, follow-face | | Modes become data on `GuiPanel` placement | Later |
| B5 | Keyboard: 2D screen or physical touch keyboard | | Reuse Horizon's controller text entry; physical keyboard later | Later |
| B6 | GUI depth/occlusion rules (HUD through walls, forced off in menus) | Many special cases | Single rule: menus never occluded, HUD configurable | Later |

## C. Input and body

| # | Vivecraft step | Weakness | Our version | Status |
|---|---|---|---|---|
| C1 | Tracked-device layer with timestamped pose histories; gesture = window query | **Wall-clock timestamps, so thresholds drift with frame rate** | `History`: fixed-step simulation time; same speed at 72/90/120 Hz (tested) | Done |
| C2 | Map runtime actions to vanilla key mappings; fake GLFW key/mouse/char events; press counters | Fragile; needs access wideners | OpenXR action set (`Controllers`) producing `Intent`; no faked input | Done |
| C3 | Dead zone, auto-sprint, snap/smooth turn (45 deg), quick torch | | `IntentMapper`: deadzone rescale, hysteresis, snap latch, hotbar flick, handedness swap | Done |
| C4 | Room-scale world model: room origin vs world, per-tick and render snapshots, rotate about head pivot, world scale | Entity follows head one tick late | `Rig`: origin/yaw/scale; `turn_about_head`; `absorb_walk` slides the rig so the head stays over the entity | Done |
| C5 | Room-scale walk: box test at head X/Z, step-up probe (10 x 0.1 m), skipped while sneaking/laddering | Head passes through walls; only a screen fade covers it | `absorb_walk` returns the residual (blocked distance) that drives the comfort fade and can push the rig out of geometry | Done (math) |
| C6 | Locomotion modes: controller/HMD/room/waist/run-in-place; fly/swim use pitch | Inertia is a hack against vanilla friction | Direction source is a parameter (`movement_to_world`); the controller owns its acceleration | Done (direction), rest Later |
| C7 | Teleport: ballistic arc, energy budget, ledge-aware destination, server limits via custom packet | Client-trusted | `teleport::trace` against a `World` trait; limits live in the server `Rules` | Done (arc), limits Designed |
| C8 | Physical sneak/crawl/jump from head height | **One calibrated height, fixed metres; standing up from a crouch can read as a jump** | `Calibration` + thresholds as fractions of the player's own height; a jump must start upright (tested) | Done (`posture`) |
| C9 | Climbing: latch per hand, pull by inverse hand delta, 0.5 m break, release launch (0.3 s, x0.66, cap 0.66), mantle | Gated to special boots; client-trusted | `Climber`: same rules, any surface the game marks climbable, launch capped by server rules | Done (latch/pull/launch), mantle Later |
| C10 | Swing to hit: tip 0.3 m ahead, speed > 2.5 m/s over 0.33 s (x1.5 creative), reach by item class, extra mining ticks | Mining multiplies vanilla ticks; client-side hit test | `SwingTracker` on `History`; `ToolClass::reach` scaled by measured arm length; mining damage returned as a number, not faked clicks | Done |
| C11 | Bow: string point, 0.15 m / 20 degree notch, aim from arrow hand to bow hand | Aim injected into vanilla throw code via look override | `Bow`: same geometry, analog tension; the game maps it to charge | Done |
| C12 | Eat/drink at mouth, backpack zone, telescope, camera trackers | Magic constants | Same pattern on `History`; | Later |
| C13 | Swim strokes, run-in-place, rowing, horse | | Same pattern | Later |
| C14 | Seated mode (keyhole turning, HMD as pointer) | A pile of special cases | A `seated` flag in the pose packet and one alternate `Intent` mapper | Later |
| C15 | Haptics: scheduler, explosion/rain trackers, hard-coded microseconds | Poorly tuned | Typed `haptics::Event` -> pulses, one table | Done (events), tuning Later |
| C16 | Full-body tracking calibration (waist, feet, elbows, knees) | | `FLAG`/capability bit reserved in the protocol | Later |

## D. Integration with the game and multiplayer

| # | Vivecraft step | Weakness | Our version | Status |
|---|---|---|---|---|
| D1 | ~150 mixins + access wideners into vanilla renderer, player, camera, GUI | **Breaks every Minecraft release**; Sodium/Iris conflicts | A library plus a small glue patch in Horizon; no mixins | Designed |
| D2 | Loader split (Fabric/Forge/NeoForge), 20+ per-mod compat mixin sets | 3 payload systems | One binary, one protocol | n/a |
| D3 | Handshake with a monotonic **version enum** plus legacy path | Every feature needs a new ordinal | Capability **bitset** negotiated by intersection (`pose_net::caps`) | Done |
| D4 | Send HMD + hand poses each tick on a side channel | | `PlayerPose` in the player's own packet: 35 bytes, smallest-three quaternions, 1/4096 m positions | Done (round-trip tested) |
| D5 | Server **temporarily rewrites the player to the hand pose** around each packet (`AimFixHandler`) and ~20 mixins read "aim pos/dir" | Race conditions; trust-based | Pose is first-class state: the server reads the hand directly; `Rules::check_hit` validates reach with tolerance | Done (validation), wiring Designed |
| D6 | Server permissions: 30+ config flags | | One `Rules` table | Done (core rules) |
| D7 | Non-VR clients see VR players as a normal avatar; VR clients render hand/head model | Custom PlayerModel hacks | Replicate the pose to all clients; IK in the avatar rig | Later |
| D8 | Vanilla servers: local-only VR, aim cosmetic | No cheating protection | Not applicable to Horizon servers; vanilla servers get head-aimed fallback | Later |

## What the improvements buy

1. **Frame-rate-independent gestures** (C1, C10): the same swing works at 72 and 120 Hz.
2. **Body-proportional thresholds** (C8, C10): children and tall adults get natural sneak depth and
   reach; no standing-up-reads-as-jump bug.
3. **No faked OS input** (B3, C2): menus and controls are real events.
4. **No pose-rewriting server hack** (D5): fewer races, and hit validation becomes anti-cheat.
5. **No per-release breakage** (D1): Vivecraft rebuilds against each Minecraft; Horizon pins one
   version and owns the code.
6. **Shared per-frame work between eyes** (A5-A7): the largest performance lever on a Quest.
