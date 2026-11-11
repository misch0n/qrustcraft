# Installing on a Quest 3

1. Turn on **Developer Mode** for your headset (Meta Horizon phone app > Devices > Headset settings).
2. Get the APK: build it with `tools/build-apk.sh smoke` (or `horizon`; see `docs/ANDROID_PLAN.md` for prerequisites), or
   download a release build once there is one. The file is `target/qrustcraft-smoke.apk` (or `-horizon.apk`).
3. Install it, either way:
   - **SideQuest**: connect the headset by USB, allow USB debugging in the headset, then drag the APK
     into SideQuest (or use its *Install APK file* button).
   - **adb**: `adb install -r target/qrustcraft-smoke.apk`.
4. Launch it from the headset: **Library > Unknown Sources > QRustCraft**.

## Stage 1 (smoke) behaviour

Each eye shows a flat colour (left blue-purple, right red-purple). Pull the **right trigger**: the
colour brightens and the right controller buzzes. If you see that, OpenXR, Vulkan, the wgpu bridge
and Touch input work on your headset. If the app closes immediately, read the log:

```sh
adb logcat -s qrustcraft
```

## Game files (stage 2 and later)

Minecraft's files cannot be put in the APK. Prepare them on your PC from your own copy of Java
Edition with Horizon, then push them:

```sh
tools/push-game-files.sh /path/to/.local/horizon-client/26.3
```

They land in `/sdcard/Android/data/dev.qrustcraft.horizon/files/.local/horizon-client/26.3`, the
Horizon APK's files folder, which is where it looks for them. Install and launch the Horizon APK once
first, so Android creates that folder for the app.
