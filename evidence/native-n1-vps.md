# Native N1 VPS Evidence

Date: 2026-10-06

Node: `vps-377a113a`

Branch: `native-engine/n1-foundation`\n\nVerified source head: `8a7a2687c1fcf44935b39a7395bab724294265cf`

## Linux runtime verification

Verified on Linux using the Servo-disabled native path.

Toolchain:

- rustc 1.98.1 (48a229cea 2026-09-01)
- cargo 1.98.1 (797e8a9bc 2026-08-05)
- Linux 7.0.0-34-generic x86_64 GNU/Linux

Passed:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- full debug test suite
- full release test suite
- native fixtures
- native Servo-independence tests
- release `native_probe`

Observed native probe:

```text
implementation=awef-native
version=n1
promotion=Experiment
platform_linux=RuntimeVerified
platform_windows=CompileVerified
platform_macos=CompileVerified
platform_android=CompileVerified
platform_ios=CompileVerified
nodes=8
scene_commands=2
native_semantics=true
production_promoted=false
artifact_sha256=5af0d0a6ce3a73faf94a7c4f7f544d49750bfddc8ed6f228a0fe2d8d3fd227f6
```

## Cross-platform compile verification

The Servo-disabled native library successfully passed `cargo check --no-default-features --lib` for:

- `x86_64-pc-windows-gnu`
- `aarch64-apple-darwin`
- `aarch64-linux-android`
- `aarch64-apple-ios`

These are **compile verification claims only**. No Windows, macOS, Android, or iOS runtime claim is made.

## N1 semantic coverage added

- AWEF-owned element attributes
- case-insensitive attribute lookup
- HTML entity decoding in text and attribute values
- void-element stack semantics
- duplicate-attribute fail-closed policy pending exact HTML duplicate-attribute conformance modeling
- deterministic native execution evidence:
  - input SHA-256
  - artifact SHA-256
  - execution SHA-256
  - mutation epoch
  - scene epoch

## Claim boundary

N1 is not a general browser engine and is not production-promoted.

Linux is runtime-verified for the current bounded native execution path.

Windows, macOS, Android, and iOS are compile-verified only.

Servo remains a development oracle/bootstrap compatibility implementation and is not required by the native N1 path.
