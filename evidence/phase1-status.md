# Phase 1 Verification Status

State: CORE-VERIFIED / SERVO-LINKAGE-INCOMPLETE / RUNTIME-BLOCKED

## Verified on VPS

The normal Phase 1 path was verified on `vps-377a113a` before the later control-channel outage.

Passed:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo test --release`
- `cargo run --release --bin phase1_bench`

Observed test result:

- 24 library tests passed
- 3 routing-corpus tests passed
- 0 failures

Observed benchmark result:

- bounded Light case: 500/500 Light, ~299,656 ns/iteration
- scripted case: 500/500 Servo route, ~4,812 ns/iteration
- Light-parser fallback case: 500/500 Servo route, ~6,184 ns/iteration
- harness peak RSS observed: ~2,536 KiB

These are harness measurements, not browser-engine superiority claims.

## Servo compile progress

The first Servo 0.6.0 compile attempt exposed missing VPS development metadata for Fontconfig.

A no-root dynamic-loading experiment bypassed discovery but proved incompatible with Servo's compile-time font API and was rejected.

A user-local dependency sysroot was then created beneath `$HOME/src/awef-sysroot`, without system-wide package installation. It contains the Fontconfig/Freetype development metadata and dependent pkg-config files required so far.

The VPS already contains `/usr/bin/llvm-objdump-21`; a user-local alias was used because SpiderMonkey expects `llvm-objdump`.

With those boundaries, Servo compilation progressed through substantial parts of SpiderMonkey and Servo, including WebRender, script, fonts traits, media, storage, embedder traits, geometry/base/url, and related crates.

Full Servo linkage did not finish before the VPS control channel became unavailable. It is therefore **not** recorded as PASS.

## New bounded verification controls

The branch now contains:

- `scripts/bootstrap-servo-vps.sh`
- `scripts/verify-phase1-vps.sh`
- `docs/VPS-VERIFICATION.md`
- `docs/SERVO-RUNTIME-PLAN.md`

The verification runner constrains the heavy Servo build with:

- `CARGO_BUILD_JOBS=1`
- `CARGO_INCREMENTAL=0`
- `nice -n 10`

It also captures exact Git/toolchain evidence and persistent logs.

## Required before merge

1. rerun the bounded VPS verification profile;
2. complete `cargo check --features servo-engine`;
3. complete `cargo test --features servo-engine`;
4. record exact Servo linkage evidence;
5. construct `SoftwareRenderingContext`;
6. construct Servo + WebView using the documented embedding API;
7. load a local/data document;
8. paint and read back a non-empty frame;
9. bind the frame hash into an execution receipt;
10. run Light-vs-Servo differential fixtures;
11. only then consider promotion or merge.

## Claim boundary

Phase 1's Light Plane, fail-closed routing, receipts, tests, and benchmark harness are verified on the VPS.

Servo 0.6.0 integration is structurally progressing but full linkage, WebView runtime initialization, first paint, readback, and comparative performance remain unverified.
