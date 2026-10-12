# Vendor CSS compatibility policy — 2026-10-12

## Decision model

This policy separates **valid but nonmatching selectors** from **invalid selectors**, **unsupported native geometry** and **optional vendor presentation**. It is not a blanket 'ignore unknown CSS' rule.

1. A nonfunctional unknown pseudo-element beginning with case-insensitive `::-webkit-` is syntactically valid for web compatibility in modern engines, but has no owned native element representation. Treat its selector arm as nonmatching; do not apply declarations to the base element. Other independently valid comma-separated arms remain effective. Reject functional forms `::-webkit-foo()` when not supported.
2. Unknown pseudo-classes, malformed selector syntax and other invalid selector-list members require standards-aware error handling; outside forgiving list contexts, a single invalid arm invalidates its entire selector rule. The current bounded compiler continues rejecting these until rule-local error recovery is explicitly verified.
3. `:is()` and `:where()` accept forgiving selector lists, with distinct specificity. `:has()` has relative selectors and cannot simply inherit every forgiving-list behavior; treat it as a separately certified capability.
4. Standard equivalents such as `::placeholder` and `::file-selector-button` require distinct supported implementations; never interpret `::-webkit-input-placeholder` as targeting the containing `input`.
5. Nonstandard scrollbar subparts (`::-webkit-scrollbar`, `-thumb`, `-track`, `-corner`, `-button`, etc.) affect presentation, but not core DOM selection. A basic native system scrollbar remains usable, and accessibility behavior must not be hidden or fabricated.
6. Standard `scrollbar-width` and `scrollbar-color` may override WebKit scrollbar styling under defined conditions. Do not translate vendor declarations blindly or prioritize them over supported standards.
7. Gecko-specific `::-moz-*`, `:-moz-*`, Microsoft `::-ms-*`, Apple-specific and legacy selector syntax must be classified independently. Never assume their syntax receives the WebKit compatibility exception.

## Required automatic negative/positive variants

- WebKit scrollbar root, thumb, track, corner; unknown future WebKit name; mixed case.
- Comma group `.x::-webkit-scrollbar, p`: preserve `p`; no accidental `.x` match.
- Unknown `::-webkit-foo()` with parentheses: must not be mistaken for the nonfunctional exception.
- Unknown standard pseudo-class; unknown vendor pseudo-class; unknown non-WebKit pseudo-element: must not be globally ignored.
- `:is()` and `:where()`: independent forgiving selector behavior and specificity evidence.
- Nested selector functions, comma/parenthesis/quote balancing; malformed CSS cannot consume the following valid rule.
- `scrollbar-width`, `scrollbar-color`, vendor styles and forced-colors interactions: defer visual claims until native scrollbar behavior exists.
- Universal selector `*` is *standard* (not vendor-specific) and needs its own governed implementation; never handle it by the vendor skip path.
- Large selectors, deep nesting, pathological comma lists and invalid Unicode: bounded parse budget, no recursion explosion, no externally fetched dependencies.

## Evidence gate

Run `scripts/verify-edge-compatibility.sh` for offline tests, matched-font Chromium tests separately, and the ignored CNET network test only in an authorized live verification environment. Record each new blocking construct with a reproducible minimized fixture and observed test result. Preserve a strict release hold whenever native scene equivalence, layout semantics, accessibility, formatting or security is unverified.

## References

- MDN CSS error handling: https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Syntax/Error_handling
- MDN WebKit-prefixed extensions: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Webkit_extensions
- MDN selector lists: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Selectors/Selector_list
- MDN scrollbar-width: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/scrollbar-width
- MDN scrollbar-color: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/scrollbar-color

Status: narrow nonfunctional WebKit selector-arm compatibility added, but the broader matrix is a roadmap, not verified functionality.
