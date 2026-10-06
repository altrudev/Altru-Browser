# Contributing to Altru Browser

Altru Browser is experimental browser-engine work. Contributions are welcome when they preserve the project boundaries rather than bypass them.

## Good contribution areas

- web-platform conformance fixtures;
- HTML/CSS parsing and semantics;
- layout and rendering correctness;
- accessibility;
- cross-platform portability;
- deterministic tests and evidence;
- memory/CPU efficiency;
- security hardening;
- documentation and reproducible build improvements.

## Before opening a pull request

Run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release
```

If your change touches an optional candidate adapter, run the relevant feature-gated tests too.

## Engineering rules

- Do not make an external library authoritative for Altru-owned browser semantics.
- Unsupported behavior must fail explicitly or route to a supported path.
- Add tests for new semantics and negative/failure cases.
- Do not add telemetry, remote execution, credentials, private Frequency state, or customer data.
- Avoid dependencies when a small owned implementation is safer and clearer.
- Keep platform APIs behind PlatformHost or another explicit portability boundary.
- Do not copy code from upstream projects with incompatible licensing or contribution restrictions.

## Pull requests

Explain:

1. what behavior changes;
2. what trust or semantic boundary is affected;
3. what tests/evidence prove it;
4. what remains unsupported;
5. whether a new dependency or platform capability is introduced.

A capability is not considered promoted merely because its code merged.
