#!/usr/bin/env bash
# Check out the pinned Horizon into third_party/horizon and apply our patches to it.
# Idempotent: re-running resets the checkout to the pin and re-applies the series.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
pin="$(tr -d '[:space:]' < "$root/third_party/horizon.pin")"
dir="$root/third_party/horizon"

if [ ! -e "$dir/.git" ]; then
  # Shallow, like a submodule checkout would be; fetch just the pinned commit.
  git init -q "$dir"
  git -C "$dir" remote add origin "${HORIZON_URL:-https://github.com/horizon-rust/horizon.git}"
fi
git -C "$dir" fetch -q --depth 1 origin "$pin"
git -C "$dir" checkout -q -f "$pin"
git -C "$dir" clean -q -fd
for p in "$root"/patches/horizon/*.patch; do
  git -C "$dir" apply --whitespace=nowarn "$p"
  echo "applied $(basename "$p")"
done
echo "Horizon $pin ready in third_party/horizon"
