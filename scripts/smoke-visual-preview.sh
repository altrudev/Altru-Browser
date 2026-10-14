#!/usr/bin/env bash
# Offscreen smoke test: proves GUI process/window exists and captures evidence.
# A screenshot is NOT a visual equivalence certificate.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v Xvfb >/dev/null || { echo "BLOCK: missing Xvfb"; exit 2; }
command -v ffmpeg >/dev/null || { echo "BLOCK: missing ffmpeg"; exit 2; }
BIN="${ALTRU_PREVIEW_BIN:-target/debug/altru-browser}"
[[ -x "$BIN" ]] || { echo "BLOCK: preview binary missing ($BIN)"; exit 2; }
OUT="${ALTRU_PREVIEW_EVIDENCE:-$PWD/evidence/visual-preview}"
mkdir -p "$OUT"
display=:"${ALTRU_VISUAL_DISPLAY:-97}"
Xvfb "$display" -screen 0 1280x820x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
xpid=$!
app_pid=''
trap '[[ -n "$app_pid" ]] && kill "$app_pid" 2>/dev/null || true; kill "$xpid" 2>/dev/null || true' EXIT
sleep 2
DISPLAY="$display" "$BIN" >"$OUT/browser.log" 2>&1 &
app_pid=$!
sleep 6
kill -0 "$app_pid" || { echo 'BLOCK: browser exited'; tail -25 "$OUT/browser.log"; exit 1; }
DISPLAY="$display" ffmpeg -hide_banner -loglevel error -f x11grab -video_size 1280x820 -i "$display" -frames:v 1 -y "$OUT/start-page.png"
test -s "$OUT/start-page.png"
sha256sum "$OUT/start-page.png" | tee "$OUT/SHA256SUMS"
echo "VISUAL_SMOKE_CAPTURED=$OUT/start-page.png"
echo 'STATUS=CAPTURE_ONLY_REQUIRES_HUMAN_OR_REFERENCE_VISUAL_REVIEW'
