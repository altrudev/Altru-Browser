# Development Guide

## Requirements

- a current stable Rust toolchain;
- Cargo;
- Git.

Optional platform/adapter experiments may require additional system dependencies.

## Build

```bash
cargo build
```

## Required verification

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release
```

Optional candidate adapter:

```bash
cargo test --features taffy-layout
```

Full native portability gate:

```bash
scripts/verify-native-platforms.sh
```

Community-release audit:

```bash
scripts/audit-community-release.sh
```

## Working rules

- add tests with behavioral changes;
- include negative/failure tests for new trust boundaries;
- do not silently widen authority;
- do not add telemetry by default;
- do not make a third-party library authoritative for Altru-owned semantics;
- keep platform-specific APIs behind explicit adapters;
- document unsupported behavior rather than approximating it silently.

## Evidence

Evidence files under `evidence/` record bounded verification results. Generated local runtime logs belong under `evidence/runtime/` and are ignored by Git.

A passing test does not by itself promote a capability to production.
