#!/usr/bin/env bash
set -euo pipefail

ROOT="${AWEF_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

targets=(
  "x86_64-unknown-linux-gnu"
  "aarch64-linux-android"
  "x86_64-pc-windows-gnu"
  "aarch64-apple-darwin"
  "aarch64-apple-ios"
)

installed="$(rustup target list --installed)"

for target in "${targets[@]}"; do
  if grep -qx "$target" <<<"$installed"; then
    echo "CHECK $target"
    RUSTFLAGS="-D warnings" cargo check --target "$target" --lib
    echo "PASS $target"
  else
    echo "PENDING $target (Rust target stdlib not installed)"
  fi
done
