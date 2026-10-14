# EDRG — cross-origin native stylesheet policy (2026-10-14)

Native-first opt-in path on Prometheus. No Servo execution, credentials or production promotion.

## Implementation
- Conventional link rel=stylesheet without crossorigin may request a cross-origin HTTPS CSS resource through the governed ResourceBroker, with bounded redirect hops, media type and byte limits. This removes the previous exact-origin-only restriction for this specific classic stylesheet mode.
- Links with crossorigin must be validated against CORS response headers; because ResourceResponse does not carry headers, this mode is currently denied rather than treated as an opaque stylesheet.
- The broker permits HTTPS origin changes on stylesheet redirects, still rejects malformed/non-HTTPS redirect URLs and enforces its hop limit. The native runtime rejects localhost and direct private/local IP references in the stylesheet manifest. Full DNS-rebinding and network isolation have not been independently verified.
- Opt-in remains OFF by default pending full CSS conformance and visual acceptance. This is not a claim of full CSP/CORS/stylesheet security support.

## Test evidence
- Governed classic cross-origin stylesheet fixture changes the resulting native text size to 27px: PASS.
- Explicit crossorigin fixture is denied pending CORS validation: PASS.
- Offline compatibility gate: PASS, 215 default library tests.
- Desktop-feature library tests: 225 passed, 0 failed.
- Native live CNET document test with opt-in OFF: PASS.
- Reference-browser screenshot parity: NOT VERIFIED for this change, previously FAIL.

## Blocking omissions
Full CSP enforcement, CORS response-header validation, CSS @import, stylesheet media matching, cross-origin resource timing, DNS-resolution pinning/SSRF controls, complete cascade interleaving, image/font retrieval and native graphical painting remain incomplete. The release stays BLOCKED. Do not silently equate the enabled opt-in with a functional production browser.
