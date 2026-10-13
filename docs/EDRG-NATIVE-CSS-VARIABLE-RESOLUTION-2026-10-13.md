# EDRG — native CSS variable substitution (2026-10-13)

Scope: Native CSS only on Prometheus. No Servo invocation or production promotion.

Implemented:
- Recursive CSS var() substitution with nested fallback terms, bounded depth and dependency-cycle detection.
- Missing and invalid CSS custom properties in supported computed-style declarations resolve to CSS unset semantics instead of terminating the complete document; inherited properties inherit, non-inherited properties return to their initial values.
- Reuse native root/current/parent font-relative calculations after custom-property substitution.
- Explicit adversarial tests for recursive variables, missing references, cycles, malformed expressions and fallback.

Observed acceptance:
- Default library: 206 passed, 0 failed.
- Full offline compatibility gate: PASS, including Taffy tests and differential fixture verifier.
- Live native CNET test was explicitly run and FAILS at UnsupportedLayout("generated pseudo content inside flex/grid formatting context"). Earlier custom-property failure was passed.
- Remote GUI, full CSS standard conformance, reference screenshots, and signed independent Frequency execution receipts: NOT VERIFIED.
- Existing strict-clippy and repository formatting debt from previous sprints remain non-green.

Release verdict: BLOCKED. The parser/cascade and value subsystem still requires full CSS syntax/value coverage, and native layout must implement pseudo-generated content in flex/grid without invented geometry. Passing component tests cannot be called complete browser support. Excluded from this change: previously isolated experimental Servo dependency modifications to Cargo.toml / Cargo.lock.
