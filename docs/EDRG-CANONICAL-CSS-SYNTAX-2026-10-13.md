# EDRG — canonical native CSS syntax and value representation (2026-10-13)

## Authority envelope
Implemented on the isolated Prometheus worktree. Only native CSS syntax, typed value representation, a read-only diagnostic probe, and integration into owned declaration parsing were changed. No Servo, credentials, external execution, release promotion or deployment.

## Implemented
- `css_canonical` provides bounded, deterministic, source-preserving declaration lists, cascade importance, declaration order, and custom-property case preservation.
- Balanced splitting avoids treating semicolons and colons within strings, functions or brackets as declaration boundaries.
- Value-component tree retains numbers, units, functions and nesting, plus raw source; unknown syntax can be preserved without manufactured semantics.
- Canonical rule tree captures ordinary style rules, nested media/supports/container/layer/keyframes rule lists, declaration-based at-rules, and explicitly unsupported raw at-rule blocks.
- The native `parse_declarations_with_translations` now consumes canonical declarations rather than splitting raw strings on semicolons.
- `css_canonical_probe` reads CSS from stdin, prints syntax inventory, and performs no network or rendering operations.
- Maximum CSS sizes, number of rules/declarations, value function depth and nested at-rule depth are bounded. Syntax error inputs return an explicit failure.

## Evidence
- `scripts/verify-edge-compatibility.sh`: PASS; **210 native library tests passed** and Taffy/differential fixtures passed.
- Independent read-only syntax probe on Prometheus successfully processed **15/15 CNET external CSS files**, about 418 KB combined.
- Syntax probe successfully processed **21/21 inline CNET <style> blocks**.
- The largest CNET stylesheet (243,691 bytes) produced 1,153 style rules, 4,386 declarations and 317 at-rules in its canonical inventory. Hundreds of at-rule bodies were preserved as unsupported raw content, not interpreted as supported semantics.

## Blocking gaps — no misleading completion claims
- This is a canonical *syntax representation* and a parsing integration, not full CSS Level 3/4 semantic conformance. Complex CSS grammar, tokenized comments, selectors, shorthands, conditions, invalid-at-computed-value-time rules, font loading, advanced CSS math, full cascade and dynamic invalidation require additional implementation and validation.
- Typed value trees currently support inspection, but the existing computed-style engine still uses its bounded CssValue compatibility layer; no single fully wired canonical value evaluator is demonstrated.
- Live CNET browser rendering remains blocked at generated pseudo-content inside flex/grid formatting contexts, and real reference-browser visual parity is unverified.
- Strict repository-wide clippy and rustfmt had existing blockers in earlier sweeps; neither is claimed green here.
- No independently signed Frequency gateway execution receipt or production deployment.

## Release decision
Canonical parsing increment is suitable for review, **not** a completed CSS engine or production-ready browser. EDRG release gate remains BLOCKED pending full semantic wiring and live reference-frame acceptance.
