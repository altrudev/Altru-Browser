# Community Release Gate

The repository is public, but a public repository is not the same thing as a community-ready release.

This gate controls release tagging, public capability claims and active community invitation.

## Required

- [x] clean public history independent from private development history;
- [x] no credentials, tokens, private customer data, or private Frequency implementation in the exported tree;
- [x] MIT OR Apache-2.0 license files and Cargo metadata agree;
- [x] README describes experimental status and capability limits;
- [x] contribution, security, governance, conduct and support policies are present;
- [x] generated build artifacts and runtime logs are excluded;
- [x] dependency/license inventory exists;
- [x] no Servo-derived source is copied into Altru-owned native implementation;
- [x] `cargo fmt --check` passes on the exact public candidate commit;
- [x] `cargo clippy --all-targets -- -D warnings` passes on the exact public candidate commit;
- [x] debug and release suites pass on the exact public candidate commit;
- [x] optional promoted feature gates pass;
- [x] native platform matrix is re-run from the exact public candidate commit;
- [x] release evidence is recorded externally against the exact commit via the verification issue/tag;

## State

All required community-foundation gates have passed: **COMMUNITY CANDIDATE ALLOWED**.

Production-readiness remains a separate, much higher gate.
