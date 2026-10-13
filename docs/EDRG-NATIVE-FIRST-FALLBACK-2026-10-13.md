# EDRG — native-first rendering / fail-closed fallback (2026-10-13)

## Authority envelope
Allowed: isolated Prometheus worktree, local dependency experiments, owned-native CSS/runtime edits, fixture execution and local evidence. No remote preview deployment, DNS changes, credentials, profile reads, or third-party engine execution.

## Implemented and exercised
- NativeRuntime always executes NativeEngineAdapter first. Fallback policy defaults to Deny.
- PermitVerifiedServo is only an authorization intent, **not** a working integration or execution path. If an unsupported CSS selector occurs, an explicitly authorized but unwired fallback is reported as FallbackUnavailable; all other native errors stay native errors. No automatic Servo invocation.
- Companion observation maps the separate failure class to fallback-unavailable, preserving visibility.
- Native CSS universal element selector * now has zero specificity and supports qualified *.class, *#id and *[attribute] syntax. Unsupported syntaxes still reject.
- The owned-native end-to-end fixture executes universal selectors, produces an artifact and SHA256-linked evidence without Servo.

## Evidence / limitations
- Focused universal selector parse and stylesheet matching tests: PASS.
- Owned-native document rendering and evidence fixture: PASS.
- Default-deny / approved-but-unwired failure mode, explicit broker, history, denied resources: PASS.
- scripts/verify-edge-compatibility.sh: PASS on Prometheus (offline default+Taffy tests, differential fixture verifier and diff check).
- cargo fmt --check: BLOCKED by pre-existing formatting drift in files outside this sprint.
- cargo clippy --offline --lib -- -D warnings: BLOCKED by seven existing lint findings; **not** an acceptance PASS.
- Full public CNET rendering and independent remote GUI acceptance: NOT VERIFIED. There may be additional HTML/CSS/JavaScript blockers.
- Servo 0.6 compilation remains experimental and is not required for native runtime. Isolated Cargo image version / lockfile edits must not be included in native-first commit until separately validated.
- DDC/Frequency execution gateway has not produced an independently signed receipt for this sprint; this file documents the EDRG checks, not a claimed gateway attestation.

## Promotion
Only native selector and fail-closed policy changes are eligible for a review PR, not production promotion. Require a new cross-platform build, UI screenshot, CNET load check, artifact provenance and adverse-case replay before release.
