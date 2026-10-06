# Phase 0 Prometheus Evidence

Date: 2026-10-05
Node: prometheus
Rust: 1.93.1
Cargo: 1.93.1

## Verification

```
cargo fmt
cargo test
cargo run --quiet -- examples/static.html
cargo run --quiet -- examples/scripted.html
```

Result: 12 tests passed, 0 failed.

## Bounded example

- route: Light
- confidence: ProvenBounded
- authoritative: false
- input SHA-256: 085229c724f0d3dda9b10767674abf149d393a2893d792d76d29fb071d2b2e83
- decision SHA-256: 4c75530c8fdf38c52f0b2e228648f224c091009e34d0e2ecfd84dc70ff7dc9f0

## Scripted example

- route: Servo
- confidence: Complex
- signal: Script
- authoritative: false
- input SHA-256: 66e0e113e18523b5d9ab4a75f7544bd9e2ebf54ed61257a3a281890c649afc5d
- decision SHA-256: 985ebb8d33a023f14f1fe73f198d094b5fa62a3c5ec27d00a89a086d270d0ece

## Claim boundary

This evidence verifies the Phase 0 routing, lifecycle, receipt, and fail-closed test harness only. It does not verify a renderer, Servo integration, browser compatibility, or performance superiority.
