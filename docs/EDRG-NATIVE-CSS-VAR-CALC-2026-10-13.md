# EDRG — computed CSS expressions with nested variables

Native-first Prometheus worktree; no Servo invocation or production deployment.

- Added deferred native length expressions when `calc(...)` contains `var(...)`, replacing immediate CSS parse failure.
- Computed-value resolution now performs bounded variable substitution, nested fallback and cycle handling before attempting existing verified linear `calc(em + px)` capability evaluation.
- Unsupported expressions remain invalid at computed-value time, falling back to CSS `unset` semantics; no arbitrary script evaluation or default numbers.
- Focused test covers two variable substitutions within a `calc` expression and an absent custom-property fallback.
- Full offline compatibility gate PASS: 213 library tests passed; Taffy and differential fixtures PASS.
- Explicit live CNET test FAILS at `UnsupportedLayout("generated pseudo content inside flex/grid formatting context")`. There is no new CSS parse failure, but this is NOT live acceptance.
- Strict clippy, full CSS conformance, reference-browser visual parity, independent signed Frequency receipt and production release remain unverified or blocked.

Release disposition: BLOCKED. This is a verified CSS computed-value integration increment, not completion of the standards-compliant CSS engine. The authoritative remote preview has not been replaced.
