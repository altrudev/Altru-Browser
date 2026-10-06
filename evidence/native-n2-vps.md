# Native N2 VPS Evidence

Date: 2026-10-06

Node: `vps-377a113a`

Branch: `native-engine/n2-style-cascade`

Verified source head: `0a5b78c9694780ab04f6ebb5a79be170768dd36a`

## N2 scope

N2 extends the Servo-independent native path with:

- AWEF-owned CSS parsing for a bounded selector/property subset;
- tag/class/id compound selector matching;
- specificity and source-order cascade;
- inline style precedence;
- inherited font size;
- inherited custom properties and bounded `var(--name)` resolution;
- `display: none` subtree suppression;
- block margins and padding;
- recursive DOM-driven layout;
- explicit mutation invalidation contracts;
- independent reftest-style scene comparisons;
- optional Taffy 0.14.0 geometry experiment behind `taffy-layout`.

## Linux verification

Passed on the VPS:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- full debug test suite: 74 passed, 0 failed
- full release test suite: 74 passed, 0 failed
- N2 CSS integration tests: 4 passed
- N2 independent reftest-style tests: 3 passed
- Taffy feature flex geometry experiment: passed
- release native probe: passed

Observed release probe:

```text
implementation=awef-native
version=n2
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
artifact_sha256=031b62a34fc364dc1bc07dda6db5b9c65fc3cbb5a0e616cbc8d6b5860a54ba74
```

## Cross-platform compile verification

The default Servo-disabled native library passed `cargo check --no-default-features --lib` for:

- `x86_64-pc-windows-gnu`
- `aarch64-apple-darwin`
- `aarch64-linux-android`
- `aarch64-apple-ios`

The feature-gated Taffy candidate also passed compile verification for all four targets.

These are compile claims only. No non-Linux runtime claim is made.

## Resource evidence

The prior Servo/SpiderMonkey build cache had expanded the Cargo `target/` directory to approximately 16.5 GiB and exhausted the VPS filesystem during the Taffy experiment.

Only reproducible Cargo build artifacts were removed with `cargo clean`:

- 24,999 generated files removed;
- approximately 16.5 GiB recovered;
- source, Git history, evidence and Cargo registry retained.

The final N2 verification completed with approximately 15 GiB filesystem headroom.

## Taffy boundary

Taffy 0.14.0 is an optional candidate only.

Verified:

- standalone Flexbox geometry probe on Linux;
- cross-platform compilation for Windows, macOS, Android and iOS.

Not claimed:

- AWEF FlexLayout capability;
- AWEF GridLayout capability;
- web-compatible flex/grid behavior.

Promotion requires wiring AWEF-resolved styles into the Taffy geometry boundary and differential/conformance evidence.

## Claim boundary

N2 is still `PromotionState::Experiment`.

It is not a general HTML/CSS compatibility claim and is not production-ready.

Linux is runtime-verified for the current bounded native N2 path. Windows, macOS, Android and iOS are compile-verified only.

Servo remains outside the native N2 execution path and is retained only as a development oracle/bootstrap compatibility implementation.
