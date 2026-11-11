#!/usr/bin/env bash
# Build a sideloadable QRustCraft APK for Meta Quest.
#
#   tools/build-apk.sh smoke     headset smoke test (OpenXR + Vulkan + Touch, no Minecraft). Default.
#   tools/build-apk.sh horizon   Horizon (Rust Minecraft) as an Android app. Needs
#                                tools/setup-horizon.sh first. A flat panel on the Quest for now.
#
# Needs: rustup target add aarch64-linux-android; cargo install cargo-ndk; the Android NDK (r26+)
# and SDK build-tools/platform via ANDROID_HOME and ANDROID_NDK_HOME; and, for `smoke`, the
# Khronos OpenXR loader (libopenxr_loader.so, arm64-v8a) at $OPENXR_LOADER (from the
# org.khronos.openxr:openxr_loader_for_android AAR on Maven Central).
#
# Output: target/qrustcraft-<variant>.apk, signed with a debug key (fine for SideQuest / adb install).
set -euo pipefail

variant="${1:-smoke}"
: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}"
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK}"

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/apk-$variant"
bt="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
platform="$(ls -d "$ANDROID_HOME"/platforms/android-* | sort -V | tail -1)"
# PROFILE=release (default) or debug; any other Cargo profile name works too.
profile="${PROFILE:-release}"
case "$profile" in debug) cargo_profile=dev ;; *) cargo_profile="$profile" ;; esac
llvm="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
# API 29 is the manifest's minSdk, and the first level with libaaudio (Horizon's audio needs it).
api=29

rm -rf "$out" && mkdir -p "$out/lib/arm64-v8a"

case "$variant" in
  smoke)
    : "${OPENXR_LOADER:?set OPENXR_LOADER to libopenxr_loader.so (arm64-v8a)}"
    manifest="$root/android/AndroidManifest.xml"; lib=libqrustcraft_quest.so
    echo "== building native library ($profile)"
    # The `android` feature links libopenxr_loader.so, so the linker must find it.
    (cd "$root" && RUSTFLAGS="${RUSTFLAGS:-} -L native=$(dirname "$OPENXR_LOADER")" \
      cargo ndk -t arm64-v8a -P $api -o "$out/jni" build -p qrustcraft-quest --profile "$cargo_profile")
    cp "$OPENXR_LOADER" "$out/lib/arm64-v8a/libopenxr_loader.so"
    ;;
  horizon)
    [ -d "$root/third_party/horizon/crates" ] || { echo "run tools/setup-horizon.sh first"; exit 1; }
    manifest="$root/android/AndroidManifest.horizon.xml"; lib=libqrustcraft_horizon.so
    echo "== building Horizon for Android ($profile): this takes a while"
    (cd "$root/app" && cargo ndk -t arm64-v8a -P $api -o "$out/jni" build --profile "$cargo_profile")
    ;;
  *) echo "unknown variant: $variant (smoke|horizon)"; exit 1 ;;
esac

cp "$out/jni/arm64-v8a/$lib" "$out/lib/arm64-v8a/$lib"
# Symbols are for the build machine, not the headset: a Horizon build is hundreds of MB with them.
"$llvm/llvm-strip" --strip-unneeded "$out/lib/arm64-v8a/$lib"

echo "== packaging"
"$bt/aapt2" link -o "$out/base.apk" -I "$platform/android.jar" \
  --manifest "$manifest" --min-sdk-version $api --target-sdk-version 32
(cd "$out" && zip -q -r base.apk lib)
"$bt/zipalign" -f -p 4 "$out/base.apk" "$out/aligned.apk"

echo "== signing (debug key)"
ks="${KEYSTORE:-$HOME/.android/debug.keystore}"
if [ ! -f "$ks" ]; then
  mkdir -p "$(dirname "$ks")"
  keytool -genkeypair -keystore "$ks" -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi
apk="$root/target/qrustcraft-$variant.apk"
"$bt/apksigner" sign --ks "$ks" --ks-pass pass:android --key-pass pass:android --out "$apk" "$out/aligned.apk"

echo "done: $apk ($(du -h "$apk" | cut -f1))"
echo "install: adb install -r $apk   (or drag it into SideQuest)"
