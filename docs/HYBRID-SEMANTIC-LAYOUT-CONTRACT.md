# DDC/Frequency — Hybrid Semantic Layout Contract v1

Status: DESIGN CANDIDATE ONLY. No runtime capability or authority is promoted by this document.
Related live observation: issue #42 — CNET `grid-template-columns: 1fr auto`.
Baseline: `d1b451ed49dfb8e90a026a6ba5d3dc11ae663a04`.

## Diagnosis

The current engine reduces CSS grid tracks to `GridColumns(u16)`, stores `ComputedStyle.grid_columns: u16`, and sends `GeometryMode::Grid { columns }` to Taffy, which assigns `fr(1.0)` to every track. A mixed `1fr auto` grid cannot be represented without losing meaning. `GeometryChild` supplies only minimum height; content-based auto column sizing cannot be trusted until intrinsic horizontal contributions are available. Treating `auto` as another `1fr` is forbidden.

## Recommended multi-hybrid architecture

1. **Canonical semantic compiler** — `grid-template-columns` lowers to ACIR `GridTrackList` with per-track kinds `Fraction(positive finite)`, `Auto`, and later bounded `FixedLength`, `MinMax`. Preserve order and explicit syntax provenance; enforce a 12-track v1 limit. Existing `repeat(N, 1fr)` lowers into the *same* IR.
2. **Owned intrinsic size oracle** — compute bounded per-child min-content/max-content inline contributions from Altru's DOM, text metrics, and box constraints. Unknown inputs return `Unmeasurable`, not zero or fabricated widths. Cache by document mutation epoch, style fingerprint, font/asset generation, and width constraint.
3. **Constrained layout solver adapter** — ACIR track list plus measured contributions go into the existing feature-gated Taffy adapter. Taffy determines geometry, not CSS meaning, cascade, privileges, or capability support. Without Taffy, reject the formatting context.
4. **Independent geometry verifier** — enforce finite/nonnegative dimensions, track ordering, gap accounting, child containment, and bounded tolerance comparisons. For mixed `auto` tracks, verify intrinsic sizing with intentionally unequal child content. Never accept `auto == 1fr` as proof. Compare with independent WPT/reftest fixtures; external browser output is diagnostic, not a substitute for standard semantics.
5. **Partial invalidation/replay** — track dependencies for style, text, fonts, viewport and subresources; recalculate only affected formatting contexts after verified translation or state change. Keep current conservative invalidation as fallback when dependency graph is incomplete.
6. **Local capability cache** — store normalized ACIR and verified fixture outcomes keyed by translator version, registry manifest hash and environment inputs. Reuse only immutable promoted translators. Unknown mappings never trigger code download or execution.
7. **Companion/Frequency observation** — classify failures by semantic family, record the exact construct, input snapshot, translation identity and before/after geometry hash. Separate observed success from verified equivalence. Local-only evidence by default.

## Alternatives screened

- **String rewrite `auto -> 1fr`: REJECT**. Inexact geometry; false compatibility.
- **Taffy-only syntax patch: REJECT as final architecture**. Content intrinsic widths and ACIR meaning are still absent.
- **Separate complete native grid solver: DEFER**. Duplicates mature solver machinery; high maintenance burden.
- **Borrow Chromium/WebKit/Gecko engine: REJECT**. Violates the independent engine boundary.
- **Unrestricted runtime code synthesis: REJECT**. Violates execution provenance and authority.
- **ACIR + owned measurement + Taffy + differential tests + replay cache: PREFERRED**. Reuses mature components while retaining native semantics and independently verifiable constraints.

## Incremental delivery sequence

**H1: Semantic core** — Introduce `GridTrack::Fraction` and `GridTrack::Auto`, normalize `1fr auto`, `auto 1fr`, `1fr 1fr`, and `repeat(N,1fr)`. Add versioned receipts and negative tests for invalid/nested syntax. Does not claim rendered correctness.

**H2: Intrinsic sizing** — Add explicit min/max inline contribution contracts for text and measured native children. Report `Unmeasurable` when unavailable. Verify font and content mutation effects.

**H3: Geometry adapter** — Carry canonical tracks through computed styles and geometry requests; map to Taffy track functions and intrinsic contributions. Test unequal-content auto tracks, fractional remainder, narrow widths, gaps, and overflow. Preserve no-Taffy rejection.

**H4: Differential proof** — Pin targeted WPT/reftest fixtures and reference renderings. Run property-based/metamorphic tests (increasing the fraction expands flexible tracks; increasing intrinsic content can affect auto tracks; reordering tracks reorders geometry). Record tolerances and counterexamples.

**H5: Live browser proof** — Rebuild isolated exact candidate and replay CNET; record new boundary and visual snapshots. No promotion based solely on advancing the first error.

**H6: Runtime fast path** — Cache promoted translations and permit targeted style/layout replay; preserve tab, scroll and state when safe. Structural parser/security changes still require reload/restart.

## Promotion proof gates

- No network, storage, script, device, or browser-engine authority added.
- Unit tests for IR syntax and validation plus negative coverage.
- Integration: parse -> computed style -> geometry -> paint.
- Distinct behavior for `1fr auto` and `1fr 1fr` with unequal intrinsic content.
- Differential fixtures and reproducibility evidence tied to exact candidate commit.
- Resource caps, overflow and invalid measurement negatives.
- Exact live CNET progression plus absence of prior regression.
- Promoted registry entry only after every gate passes.

## Broader reuse

The intrinsic measurement oracle and dependency-aware replay should be reusable for flexbox, tables, pseudo-element text fragments and eventually SVG text. The ACIR track list can generalize to rows, implicit tracks and `minmax` only as independently verified extensions. Companion should rank candidate families by measured corpus coverage improvement, regression risk, authority impact and cold/warm computation cost; predicted gains are never labeled measured outcomes.
