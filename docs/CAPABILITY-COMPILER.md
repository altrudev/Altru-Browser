# ACIR + Capability Compiler

Altru Browser does not need to implement every external syntax directly.

The **Altru Capability Intermediate Representation (ACIR)** is the owned semantic layer between external technology and native browser behavior. The **Capability Compiler** recognizes bounded external constructs and translates them into ACIR primitives that the native engine already understands.

## First implemented translation

CSS media conditions are translated into deterministic environment predicates.

Supported bounded subset:

- `min-resolution: <dpi|dppx>`
- `max-resolution: <dpi|dppx>`
- `min-width: <px>`
- `max-width: <px>`, including fractional thresholds
- `prefers-reduced-motion` / `prefers-reduced-motion: reduce|no-preference`
- boolean `and` composition
- leading `not` negation
- optional `screen and` / `all and` prefix

The first live compatibility corpus showed ten distinct CNET media-query forms. They reduce to three ACIR concepts: numeric environment bounds, boolean conjunction, and preference/negation semantics. The compiler implements those reusable concepts rather than ten site-specific exceptions.

Example:

```
@media (min-resolution:192dpi) {
  .retina { font-size: 24px; }
}
```

becomes an ACIR environment predicate equivalent to:

```
resolution_dpi >= 192
```

The rule body is lowered into the existing native CSS pipeline only when the predicate is true.

## Boundary

Translation is not authority.

- unsupported media features fail closed;
- ACIR does not execute arbitrary code;
- no network or resource authority is widened;
- the compiler only lowers explicitly recognized semantics;
- native browser components remain the execution authority;
- future translations must carry their own evidence and tests.

## Direction

The same pattern can later absorb additional capability families:

- CSS conditionals and selectors;
- layout semantics;
- resource policies;
- SVG primitives;
- accessibility semantics;
- bounded Web API operations;
- protocol/schema translation.

The intended loop is:

`observe -> identify semantics -> translate to ACIR -> verify equivalence -> promote translation -> reuse`
