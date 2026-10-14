# EDRG: Governed external stylesheet ingestion — 2026-10-14

## Authority and scope
Native-first development on isolated Prometheus. This change introduces a **disabled-by-default** broker-mediated stylesheet path for the desktop-preview feature. Servo is not invoked, and production is not promoted.

## Executed implementation
- `NativeRuntime::set_external_css_enabled(true)` explicitly enables an owned, same-origin HTTPS stylesheet retrieval path.
- The source-order DOM dependency manifest selects `rel=stylesheet` references. Base-relative references are normalized with the URL parser; credentials, non-HTTPS, cross-origin, excessive count, excessive bytes, mismatched media types and broker failures are rejected.
- Every stylesheet is acquired via `ResourceRequest { kind: StyleSheet }` and an explicit broker, not ambient renderer network access. The GUI broker now accepts `text/css` only for the stylesheet kind and rejects stylesheet redirects to another origin.
- Broker-supplied CSS is compiled into native rules before computed style/layout, and a composite CSS source hash is included in native execution evidence.
- Default navigation and existing tests remain unchanged unless the opt-in flag is explicitly set.
- A desktop-feature fixture proves an approved stylesheet changes a 16px native text run to 27px, and cross-origin stylesheet references fail closed.

## Gates observed
- `scripts/verify-edge-compatibility.sh`: PASS, 215 default library tests.
- `cargo test --offline --features desktop-preview --lib`: PASS, 225 library tests.
- Explicit native live CNET document load: PASS with external stylesheet opt-in OFF.
- Full Chromium vs native screenshot comparison: previously FAILED; **not rerun against a new graphical implementation**.

## Blockers and omissions
- The opt-in path currently allows only same-origin stylesheets. CNET commonly serves CSS across resource hostnames, and complete author-origin policy, CORS/CSP checks, integrity metadata, imports, media attributes, external/inline interleaving, content decompression robustness, data: policy, additional CSS syntax and full layout semantics are not yet complete.
- The current native engine's supported CSS subset may reject whole external stylesheets; opt-in success on a bounded fixture does not establish CNET CSS compatibility.
- No images, fonts, backgrounds, clipping, colour, GPU compositing or JavaScript DOM runtime were added here.
- Strict repository-wide clippy/format baseline, independently signed Frequency receipt and pixel-level reference comparison are not claimed green.
- Prior experimental Servo Cargo.toml/Cargo.lock modifications are **excluded** from this change.

## Release decision
BLOCKED. Verified integration of a new governed external-stylesheet adapter, not a completed browser. Next step: complete CSS source-order cascade and supported-property coverage using fixture-driven reference comparisons, then visual scene/paint resources.
