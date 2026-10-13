# EDRG edge sprint — native CSS math and author cascade (2026-10-13)

## Scope and authority
Isolated Prometheus review worktree only. Native engine remains authoritative. Servo remains optional, disabled by default, and not executed by these changes. No production promotion, no credential/profile access, no remote desktop deployment, no external engine execution.

## Implemented native semantics
- A bounded ACIR linear length with an exact thousandths-based font-relative factor and pixel offset. A verified translation registry entry admits only nonnegative `calc(Nem + Mpx)` and binds the source through SHA-256 translation receipts.
- Font-size calculations use the parent font. Padding, margins and gap calculations use the resolved current font; all supported values resolve to pixels before style application.
- Author-origin `!important` is parsed as declaration metadata, not appended to values. The cascade preserves source/specificity order within normal and important tiers; important declarations override normal declarations, including inline normal declarations. Inline important declarations override stylesheet important declarations. Pseudo-element and generated-content paths use the same ordering.
- Exact `gap: 0 0` is lowered to zero for both axes with a verified receipt. Other unequal or unsupported gap pairs remain unadmitted.

## Evidence
- Compiler adversarial tests reject subtraction, nested expressions, variables in calc, reversed units, multiple operands, over-precision and excessive lengths.
- Pixel-resolution test: `calc(0.667em + 2px)` uses the appropriate font basis. Distinct source values yield different receipt-bound execution hashes.
- Author/inline-important ordering tests: PASS.
- Owned-native end-to-end HTML/CSS execution and receipt tests: PASS.
- `scripts/verify-edge-compatibility.sh`: PASS on Prometheus. Default library suite: **203 passed, 0 failed**; offline Taffy suite, fixture verifier and git diff check also passed.
- Clippy strict `-D warnings`: BLOCKED by the same seven pre-existing lint findings as before this sprint. Repository-wide rustfmt check remains blocked by unrelated existing drift. Neither is an acceptance pass.

## Live-site adversarial progression
The ignored live CNET native-runtime test was executed explicitly, not counted as a successful regression:
1. Prior: unsupported universal selector `*`.
2. After native universal support: `padding-top: calc(0.667em + 2px)`.
3. After bounded calc support: `font-size: var(--wp--preset--font-size--small) !important`.
4. After important cascade support: `gap: 0 0`.
5. After verified zero-pair lowering: **`margin-top: 2.25rem`**, still unsupported.

**RELEASE GATE: BLOCKED.** General CNET rendering and remote GUI use are not verified. Native `rem` needs root-font semantics, not a hardcoded guess; further selectors, scripting and resource gaps may follow. No independent Frequency gateway signature or cross-platform remote visual receipt was produced; this is local EDRG test evidence only.

## Isolation
Experimental `image` crate 0.25.10 and lockfile changes relate solely to the unfinished Servo build and are expressly excluded from this native CSS change. No production backend switch is authorized.
