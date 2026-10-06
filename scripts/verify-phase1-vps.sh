#!/usr/bin/env bash
set -euo pipefail

ROOT="${AWEF_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SYSROOT="${AWEF_SYSROOT:-$HOME/src/awef-sysroot}"
EVIDENCE_DIR="$ROOT/evidence/runtime"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="$EVIDENCE_DIR/vps-$STAMP"

mkdir -p "$OUT"

export PATH="$HOME/.local/bin:$PATH"
source "$ROOT/scripts/servo-hybrid-abi-env.sh"

# DDC/Frequency promotion: one job kept the VPS healthy with several GiB of
# headroom, so two jobs are now permitted. The heavy gate remains low-priority.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export CARGO_INCREMENTAL=0

cd "$ROOT"

run() {
  name="$1"
  shift
  printf '== %s ==\n' "$name" | tee "$OUT/$name.log"
  /usr/bin/time -v "$@" >>"$OUT/$name.log" 2>&1
}

run fmt cargo fmt --check
run clippy cargo clippy --all-targets -- -D warnings
run test-debug cargo test
run test-release cargo test --release
run bench cargo run --release --bin phase1_bench

run servo-check nice -n 10 cargo check --features servo-engine
run servo-runtime-test nice -n 10 cargo test --features servo-engine --test servo_runtime
run servo-runtime-probe nice -n 10 cargo run --features servo-engine --bin servo_runtime_probe

sha256sum Cargo.toml Cargo.lock > "$OUT/manifest.sha256"
git rev-parse HEAD > "$OUT/git-head.txt"
rustc --version > "$OUT/toolchain.txt"
cargo --version >> "$OUT/toolchain.txt"

printf 'Verification complete: %s\n' "$OUT"
