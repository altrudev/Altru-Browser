# EDRG native paint integration evidence — 2026-10-16

## Change
- Extended native computed style for inherited text color and per-element background color.
- Added basic RGB/hex/named color lowering in the cssparser intake boundary; these are deliberately bounded native-supported values, not full CSS Color 4.
- Native layout emits colored background rectangles using computed styles and known element geometry.
- Native scene now carries Fill and ColoredText commands, consumed by the Altru egui desktop painter. No Chromium, Servo or WebView rendering fallback.
- Added a regression asserting author stylesheet -> computed color -> layout fragment -> paint scene, including inherited text color.

## Evidence
- `cargo test --offline --lib`: 218 passed.
- `cargo test --offline --features desktop-preview --bin altru-browser`: 5 passed; 2 live tests ignored by default.
- `scripts/verify-edge-compatibility.sh`: EDGE_COMPATIBILITY_OFFLINE_PASS.
- Explicit ignored live CNET document execution smoke: PASS. This does not prove correct graphical rendering.

## Blocking gaps
- No screenshot differential acceptance for this revision.
- Partial CSS Color support; no complete computed styles, images, font shaping, arbitrary clipping, paint stacking contexts, compositing or JavaScript DOM execution.
- Background height derives from bounded layout fragment geometry and is not yet equivalent to full CSS background painting.
- No artifact is promoted or attested as a finished standards-compliant browser.

Release decision: **BLOCKED** until native visual and interaction acceptance gates pass.
