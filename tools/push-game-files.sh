#!/usr/bin/env bash
# Prepare Minecraft's files on your PC with Horizon, then copy them to the headset.
#
# Horizon's preparation runs Mojang's own exporters in Java, which a Quest does not have, and
# Mojang's files may not be shipped inside an APK. So preparation happens here, from YOUR copy of
# Minecraft, and the result is pushed to the app's private folder over adb.
#
# Usage: tools/push-game-files.sh /path/to/horizon-prepared/26.3
#   (the folder Horizon's launcher or `horizon-prepare` writes: .local/horizon-client/26.3)
set -euo pipefail
src="${1:?path to the prepared 26.3 folder}"
# The Horizon APK (android/AndroidManifest.horizon.xml). It runs in its own files folder and looks
# for the prepared files at `.local/horizon-client/26.3` there, as on desktop.
pkg="${PKG:-dev.qrustcraft.horizon}"
dst="/sdcard/Android/data/$pkg/files/.local/horizon-client/26.3"

command -v adb >/dev/null || { echo "adb not found (install Android platform-tools)"; exit 1; }
adb get-state >/dev/null || { echo "no headset: enable Developer Mode and connect over USB or adb wifi"; exit 1; }
[ -d "$src" ] || { echo "not a folder: $src"; exit 1; }
[ -f "$src/horizon-resources.json" ] || echo "warning: no horizon-resources.json in $src; is this the prepared 26.3 folder?"

adb shell mkdir -p "$dst"
adb push --sync "$src/." "$dst/"
echo "pushed $(du -sh "$src" | cut -f1) to $dst"
