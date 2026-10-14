#!/usr/bin/env bash
# Deterministic offline compatibility checks; no network or deployment.
set -euo pipefail
cd "$(dirname "$0")/.."
export TMPDIR="${TMPDIR:-$HOME/.cache/altru-edge-tmp}"
mkdir -p "$TMPDIR"
cargo test --offline --quiet
cargo test --offline --features taffy-layout --quiet
python3 -m unittest discover -s fixtures/grid-differential -p 'test_verify_reference.py' -q
git diff --check
echo 'EDGE_COMPATIBILITY_OFFLINE_PASS'
