# CSS vendor matrix — implementation acceptance

Implemented in `src/vendor_css.rs` and enforced at `native_css` stylesheet admission. Classification covers Standard, WebKit, Gecko, Microsoft, Opera and Unknown selectors. Outcomes are NativeCandidate, InertWebKitPseudo and Unsupported. It recognizes the CSS Selectors Level 4 exception for nonfunctional `::-webkit-*` pseudo-elements (including selectors without an explicit base) and rejects functional vendor extensions that lack support. Other vendor-prefixed pseudo-classes/elements are not silently translated into standard styles.

The decision is intentionally **not** full rendering support. CSS rule-level invalid selector list recovery, nested forgiving selectors, detailed vendor-specific scrollbar rendering, `:-webkit-autofill` semantics, standards equivalents, accessibility, and universal-selector semantics are outstanding. The current CSS parser still aborts on some unsupported rules where a standards-conforming browser would discard only the offending rule. These must remain visible as release blockers rather than being misrepresented as compatibility.

Automatic offline regression: `scripts/verify-edge-compatibility.sh`. Run exact live CNET replay separately. Vendor matrix unit tests cover prefixes, unknown future WebKit names, functional negative forms, and the selector length bound. Release held pending broader correctness and cross-browser reference comparisons.

References: https://drafts.csswg.org/selectors/#compat ; https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Syntax/Error_handling ; https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Selectors/Selector_list
