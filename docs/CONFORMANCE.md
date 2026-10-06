# Conformance Policy

Altru Browser treats web standards and standards-derived tests as authoritative.

## Sources of truth

Primary conformance work should be grounded in:

- WHATWG HTML;
- CSS specifications;
- Web Platform Tests (WPT);
- ECMAScript specifications;
- Test262 for JavaScript behavior;
- relevant accessibility and platform specifications.

Other browsers and engines may be used for differential diagnosis, but matching another implementation is not itself proof of correctness.

## Promotion

A capability may be promoted only when:

- supported behavior is defined;
- unsupported behavior is explicit;
- positive tests pass;
- negative/failure tests pass;
- differential or standards-derived evidence exists where appropriate;
- platform effects are understood;
- evidence is bound to the implementation revision.

## No silent approximation

When behavior cannot be implemented faithfully within the supported subset, the engine must fail explicitly or use a documented supported path rather than quietly inventing behavior.
