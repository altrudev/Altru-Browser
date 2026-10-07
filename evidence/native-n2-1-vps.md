# Native N2.1 Flex/Grid Evidence

## Scope

N2.1 connects Altru-owned CSS semantics to an Altru-owned layout geometry contract and uses Taffy only as an optional geometry adapter.

This is a bounded experimental slice, not a claim of full CSS Flexbox or Grid conformance.

## Owned semantics

The native CSS/style path now owns and validates:

- `display: flex`;
- `display: grid`;
- `flex-direction: row | column`;
- non-negative pixel `gap`;
- bounded grid templates expressed as 1–12 equal `1fr` tracks.

Unsupported grid track syntax fails explicitly rather than being silently approximated.

## Geometry boundary

The public engine contract defines neutral:

- `GeometryMode`;
- `GeometryRequest`;
- `GeometryChild`;
- `GeometryResult`;
- `GeometryBox`.

No Taffy type appears in the DOM, CSS, computed-style, or native-layout semantic contracts.

With `taffy-layout` disabled, a document that requires flex/grid geometry returns `CssError::UnsupportedLayout`. This is intentional fail-closed behavior.

With `taffy-layout` enabled, the adapter translates the owned geometry request into Taffy and returns neutral geometry boxes to the native layout engine.

## Tests added

- bounded flex/grid declaration parsing;
- rejection of unsupported grid track syntax;
- computed-style propagation of flex/grid semantics;
- fail-closed flex/grid behavior without the geometry adapter;
- owned flex geometry adapter test;
- owned two-column grid geometry adapter test;
- end-to-end flex DOM/CSS → layout positioning;
- end-to-end grid DOM/CSS → second-row positioning;
- author stylesheet vs inline flex semantic equivalence;
- deterministic repeated grid layout.

## Promotion boundary

N2.1 remains **Experimental / Conformance Partial**.

Not yet claimed:

- complete Flexbox alignment/wrapping/basis/order semantics;
- arbitrary Grid track sizing, named lines, auto-placement controls, spanning, subgrid, or masonry;
- WPT-level Flexbox/Grid conformance;
- production browser readiness.

A future promotion requires standards-derived fixtures/WPT coverage and broader semantics, not just successful Taffy geometry.
