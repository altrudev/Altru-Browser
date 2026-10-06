# Altru Browser Community Foundation Evidence — 2026-10-06

## Public candidate

Repository: `altrudev/Altru-Browser`

Clean-room public foundation commit tested locally:

`4c182340755bd4f3a2de957b2443ea1ff2f97142`

The public repository has an independent Git history. Private experimental branch history was not copied.

## Boundary checks

- required community documentation: PASS
- dual MIT / Apache-2.0 metadata: PASS
- generated build/runtime artifacts excluded: PASS
- high-confidence secret scan of staged public tree: PASS
- public-history secret scan before first commit: PASS
- private-assurance boundary scan: PASS
- hard-coded private checkout paths in public verification scripts: found and repaired before publication

## Exact public-commit verification on Prometheus

- `cargo fmt --check`: PASS
- `cargo clippy --all-targets -- -D warnings`: PASS
- `cargo test`: 74 passed, 0 failed
- `cargo test --release`: 74 passed, 0 failed
- `cargo test --features taffy-layout`: 75 passed, 0 failed

## Cross-platform verification

The equivalent native-engine source lineage previously passed Linux runtime and Windows/macOS/Android/iOS compile verification on the VPS.

A fresh cross-target run against the exact public repository commit is still required before the community release gate is marked ALLOW because Prometheus does not currently have `rustup` and the VPS went offline during the publication run.

State: **PUBLIC / EXPERIMENTAL / COMMUNITY RELEASE BLOCKED ON EXACT CROSS-TARGET RECHECK**.

This is intentionally not represented as a production-readiness result.
