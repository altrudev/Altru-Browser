# Phase 1 — Light Renderer + Servo Linkage

## Objective

Turn Phase 0 routing into a measurable hybrid-engine experiment without modifying Servo.

## Light plane

Phase 1 introduces a real deterministic renderer for a deliberately narrow subset:

- structural HTML only;
- allowlisted tags;
- no tag attributes;
- no CSS;
- no JavaScript;
- no media, canvas, WebGL, frames, forms, or embedded objects;
- balanced markup required.

Accepted documents are converted to an internal display-command stream and rendered to an SVG artifact.

This is intentionally not a claim of HTML/CSS compatibility. The subset is narrow enough that we can reason about it completely and expand it only after differential tests exist.

## Two-stage fail-closed routing

1. Capability classifier rejects known active/complex capabilities.
2. Light parser validates the bounded syntax.
3. Any Light parser rejection changes the final route to Servo and records `UnsupportedLightSyntax`.
4. A decision receipt is produced only after the final route is known.

This prevents a classifier false negative from silently becoming a Light execution.

## Servo boundary

Servo is pinned as an optional dependency at exactly `0.6.0`.

The `servo-engine` feature has a compile-time linkage probe for:

- `Servo`
- `ServoBuilder`
- `WebView`
- `WebViewBuilder`
- `SoftwareRenderingContext`
- `OffscreenRenderingContext`

Phase 1 does not claim the Servo runtime is initialized yet. Runtime readiness requires an event-loop waker, rendering context, WebView delegate, paint/present loop, and navigation test.

## Required verification

Default build:

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo test --release
    cargo run --release --bin phase1_bench

Servo linkage:

    cargo check --features servo-engine
    cargo test --features servo-engine

Servo runtime integration remains blocked from promotion until an offscreen page can be loaded and painted with observable evidence.

## Promotion gates

The Light plane may remain enabled only when:

- malformed or unsupported input always falls back;
- the final receipt reflects the actual selected plane;
- SVG output is deterministic for identical input;
- differential fixtures match expected text/order/structure;
- benchmarks are recorded but not generalized beyond their tested workload.

Servo runtime may be promoted only after actual load/paint evidence exists.

## Upstream boundary

No Servo source files are modified, patched, forked, or submitted upstream by this project.
