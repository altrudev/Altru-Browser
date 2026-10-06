# Native N1 VPS Evidence — 2026-10-06

Environment: `vps-377a113a`

Verification profile: clean independent checkout, Servo feature disabled.

Current native pipeline:

```text
AWEF HTML subset
    -> AWEF DOM
    -> AWEF computed style
    -> AWEF layout fragments
    -> AWEF retained scene
    -> deterministic renderer
    -> SHA-256 evidence
```

Commands:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --quiet --bin native_probe
bash scripts/audit-native-portability.sh
cd fixtures/native-n1 && sha256sum -c MANIFEST.sha256
```

Observed results:

- rustfmt: PASS
- Clippy: PASS
- library tests: 49 passed, 0 failed
- native fixture tests: 4 passed, 0 failed
- native-independence integration tests: 2 passed, 0 failed
- existing routing corpus: 3 passed, 0 failed
- content-addressed fixture manifest: 4/4 PASS
- browser navigation/history kernel: PASS
- explicit ResourceBroker deny-by-default boundary: PASS
- NativeRuntime navigation -> broker -> native execution path: PASS
- portability audit: PASS
- target matrix: Linux PASS, Android PASS, Windows/macOS/iOS PENDING
- Servo runtime test under default/no-Servo feature: 0 tests, as expected
- no `servo-engine` feature enabled

Total executed tests in the native/default path: 58 passed, 0 failed.

Native probe:

```text
implementation=awef-native
version=n1
promotion=Experiment
platform_linux=RuntimeVerified
platform_windows=Planned
platform_macos=Planned
platform_android=CompileVerified
platform_ios=Planned
nodes=8
scene_commands=2
native_semantics=true
production_promoted=false
artifact_sha256=5af0d0a6ce3a73faf94a7c4f7f544d49750bfddc8ed6f228a0fe2d8d3fd227f6
```

Portability audit:

```text
PASS: native semantic core is platform-contract-only
```

Fixture hashes are pinned in `fixtures/native-n1/MANIFEST.sha256`.

Claim boundary:

This proves that the current N1 native document/style/layout/scene path compiles, tests, executes and emits deterministic evidence without enabling Servo.

Linux is runtime-verified for this N1 evidence. Android (`aarch64-linux-android`) is compile-verified with warnings denied. Windows, macOS and iOS remain planned/pending until their target libraries and platform-specific verification are run.

This does not prove general HTML/CSS/Web API conformance, graphical GPU rendering, JavaScript compatibility, or production readiness.

Promotion remains `Experiment`.
