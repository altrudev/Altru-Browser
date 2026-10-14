# EDRG: Native visual pipeline dependency sprint — 2026-10-14

## Observed problem
At commit `bf8767f`, native CNET HTTPS navigation and HTML text display pass on the isolated VPS; a screenshot at `/home/ubuntu/altru-visual-capture-20261014/cnet-native-final.png` shows unstyled vertically stacked text, whereas Chromium on Prometheus `/home/triangulatorium/cnet-reference-check/cnet.png` shows a responsive header, imagery, and grid. This is a **visual FAIL**, not success. Screenshots are on different hosts/times, so no defensible pixel-error metric exists yet.

## Evidence-first diagnosis
- NativeRuntime::fetch_navigation fetches only ResourceKind::Document.
- execute_native_document builds CSS from `<style>` text in the HTML. External `<link rel=stylesheet>` stylesheets are not fetched/applied.
- Native scene commands are text and unfilled rectangles; backgrounds, borders, images and web fonts are not fully represented.
- Native GUI paints that limited scene through egui; it does not provide browser-equivalent pixels.
- No JS DOM execution or reference visual-differential acceptance has passed.

## Multi-hybrid architecture (one native authoritative engine)
1. **Source parser + semantic IR**: retain the owned canonical CSS/DOM representation, proven selectors and computed-style implementation. Use vetted parsing, Taffy for bounded layout calculations and a real image/font decoding library behind strict adapters; do not delegate primary rendering or authority to Servo.
2. **Governed Resource DAG**: DOM discovery -> normalized HTTPS URL policy, per-kind limits, redirect/origin/CSP decisions, authorization -> broker fetch -> hash/content-type evidence -> decode. Stylesheets must be fetched, ordered and cascaded before layout; images/fonts must be decoded via resource adapters. No ambient network access in the engine; avoid arbitrary CSS `url()`/JS execution.
3. **Native retained scene**: text glyph runs, image textures, filled backgrounds, borders, clipping and compositing, then egui/wgpu-compatible frontend. Keep layout engine independent of GUI. Invalidation follows DOM/style/font/viewport/resource epochs.
4. **Differential observability**: Chromium reference at identical viewport/DPR/frozen time/network fixture, matching native screenshot, DOM geometry and resource manifest; reproducible comparison with image-region diagnostics. No screenshot-based pass unless corresponding native frame is captured and evaluated.
5. **EDRG enforcement**: format/clippy, full unit/integration/adversarial suite, live HTTPS broker cases, fail-closed resource policy, no hidden fallback, independent observation hashes and reproducible machine evidence. Production blocked until required gates pass.

## This sprint
- Implemented `native_dependencies::discover_dependencies` to extract source-order style/script/image/font-preload/media references from owned DOM nodes, without fetching or authorizing them.
- Wired discovered manifest into `NativeExecution.dependencies` for downstream gated resource scheduling and audit.
- Unit tests cover link rel tokenization, empty/inert sources and ordering.
- Existing unrelated Servo experiment modifications in Cargo.toml/lock are excluded from this change.

## Important limitations
Manifest currently preserves raw relative references and is not an approved URL resolver. Dynamic imports, srcset, CSS URL dependencies, media queries, preload consumption, cross-origin policy, content-type validation, image decode and stylesheet cascade wiring remain **NOT IMPLEMENTED**. Resource manifest alone does not improve rendered pixels and must not be presented as completed visual compatibility.

Release decision: **BLOCKED (visual parity and complete governed resource pipeline unverified)**.
