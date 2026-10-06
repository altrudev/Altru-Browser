#!/usr/bin/env bash
set -euo pipefail

ROOT="${AWEF_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TARGETS=(
  x86_64-pc-windows-gnu
  aarch64-apple-darwin
  aarch64-linux-android
  aarch64-apple-ios
)

cd "$ROOT"

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test native_
cargo test --test native_fixtures --test native_independence --test native_css_n2 --test native_reftest_n2
cargo test --release --test native_css_n2 --test native_reftest_n2
cargo run --release --bin native_probe
cargo test --features taffy-layout native_layout_taffy

for target in "${TARGETS[@]}"; do
  rustup target add "$target"
  cargo check --no-default-features --lib --target "$target"
  cargo check --no-default-features --features taffy-layout --lib --target "$target"
done

printf 'Native platform verification complete.\n'
