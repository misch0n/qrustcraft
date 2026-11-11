#!/usr/bin/env bash
# Build a sideloadable QRustCraft APK for Meta Quest.
#
# Needs (not available in every sandbox; see docs/ANDROID_PLAN.md):
#   - Rust with the aarch64-linux-android target:   rustup target add aarch64-linux-android
#   - cargo-ndk:                                    cargo install cargo-ndk
#   - Android NDK (r26+) and SDK build-tools (aapt2, zipalign, apksigner), via ANDROID_HOME and
#     ANDROID_NDK_HOME
#   - the Khronos OpenXR loader for Android (libopenxr_loader.so, arm64-v8a) at
#     $OPENXR_LOADER, from the org.khronos.openxr:openxr_loader_for_android AAR on Maven Central
#
# Output: target/qrustcraft-quest.apk, signed with a debug key (fine for SideQuest / adb install).
set -euo pipefail

: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}"
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK}"
: "${OPENXR_LOADER:?set OPENXR_LOADER to libopenxr_loader.so (arm64-v8a)}"

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/apk"
bt="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
platform="$(ls -d "$ANDROID_HOME"/platforms/android-* | sort -V | tail -1)"
profile="${PROFILE:-release}"

rm -rf "$out" && mkdir -p "$out/lib/arm64-v8a"

echo "== building native library ($profile)"
(cd "$root" && cargo ndk -t arm64-v8a -o "$out/jni" build -p qrustcraft-quest --"$profile")
cp "$out"/jni/arm64-v8a/libqrustcraft_quest.so "$out/lib/arm64-v8a/"
cp "$OPENXR_LOADER" "$out/lib/arm64-v8a/libopenxr_loader.so"

echo "== packaging"
"$bt/aapt2" link -o "$out/base.apk" -I "$platform/android.jar" \
  --manifest "$root/android/AndroidManifest.xml" --min-sdk-version 29 --target-sdk-version 32
(cd "$out" && zip -q -r base.apk lib)
"$bt/zipalign" -f -p 4 "$out/base.apk" "$out/aligned.apk"

echo "== signing (debug key)"
ks="${KEYSTORE:-$HOME/.android/debug.keystore}"
if [ ! -f "$ks" ]; then
  mkdir -p "$(dirname "$ks")"
  keytool -genkeypair -keystore "$ks" -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi
"$bt/apksigner" sign --ks "$ks" --ks-pass pass:android --key-pass pass:android \
  --out "$root/target/qrustcraft-quest.apk" "$out/aligned.apk"

echo "done: $root/target/qrustcraft-quest.apk"
echo "install: adb install -r target/qrustcraft-quest.apk   (or drag it into SideQuest)"
