#!/usr/bin/env bash
set -euo pipefail

ROOT="${AWEF_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

core_files=(
  src/engine_api.rs
  src/native_dom.rs
  src/native_engine.rs
  src/native_scene.rs
  src/platform.rs
  src/script_engine.rs
)

if grep -nE 'cfg\s*\(\s*target_os|cfg_attr\s*\(\s*target_os' "${core_files[@]}"; then
  echo "FAIL: native semantic core contains target_os branching" >&2
  exit 1
fi

if grep -nE 'use[[:space:]]+(x11|wayland|windows|winapi|objc2|cocoa|ndk|android_activity)::' "${core_files[@]}"; then
  echo "FAIL: native semantic core imports a platform implementation crate" >&2
  exit 1
fi

echo "PASS: native semantic core is platform-contract-only"
