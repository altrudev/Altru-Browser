#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() { printf 'BLOCK: %s\n' "$*" >&2; exit 1; }
pass() { printf 'PASS: %s\n' "$*"; }

scripts/audit-community-release.sh
pass "community release boundary"

cargo clippy --features desktop-preview --bin altru-browser -- -D warnings
pass "desktop preview clippy"

cargo test --features desktop-preview --bin altru-browser
pass "desktop preview tests"

cargo test native_runtime::tests::back_forward_and_reload_reuse_existing_history_entries
pass "owned navigation history test"

cargo build --release --features desktop-preview --bin altru-browser
pass "Linux release build"

if ldd target/release/altru-browser | grep -q 'not found'; then
  ldd target/release/altru-browser >&2
  fail "runtime shared library missing"
fi
pass "runtime shared libraries resolved"

sha="$(sha256sum target/release/altru-browser | awk '{print $1}')"
size="$(stat -c '%s' target/release/altru-browser)"
printf 'preview_sha256=%s\n' "$sha"
printf 'preview_bytes=%s\n' "$size"
printf 'candidate_commit=%s\n' "$(git rev-parse HEAD)"
printf 'ALLOW-PREVIEW: Linux developer preview audit passed\n'
