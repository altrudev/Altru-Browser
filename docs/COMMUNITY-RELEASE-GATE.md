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
- [ ] `cargo fmt --check` passes on the exact public candidate commit;
- [ ] `cargo clippy --all-targets -- -D warnings` passes on the exact public candidate commit;
- [ ] debug and release suites pass on the exact public candidate commit;
- [ ] optional promoted feature gates pass;
- [ ] native platform matrix is re-run from the exact public candidate commit;
- [ ] release evidence records the exact public commit hash.

## State

Until every runtime/verification item above is bound to the exact public commit: **PUBLIC / EXPERIMENTAL / RELEASE BLOCKED**.

After all items pass: **COMMUNITY CANDIDATE ALLOWED**.

Production-readiness is a separate, much higher gate.
