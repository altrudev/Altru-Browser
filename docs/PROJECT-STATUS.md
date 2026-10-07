# Project Status

**Status:** Experimental / community foundation.

Altru Browser is suitable for engineering collaboration, architecture review, conformance work and bounded experimentation.

It is not yet suitable as a production replacement for established browsers.

## Verified baseline

- deterministic native execution for bounded fixtures;
- 74-test default debug suite;
- 74-test default release suite;
- 75-test suite with the optional Taffy feature;
- Linux native runtime verification;
- Windows compile verification;
- macOS compile verification;
- Android compile verification;
- iOS compile verification.

See `evidence/native-n2-vps.md` for the current evidence snapshot.

## Experimental N2.1 layout slice

The development branch includes bounded Altru-owned flex/grid semantics behind a neutral geometry contract. Taffy is optional and geometry-only. Full Flexbox/Grid conformance remains unclaimed. See `evidence/native-n2-1-vps.md`.

## Explicitly not claimed

- full HTML/CSS/Web API support;
- complete JavaScript compatibility;
- production sandbox maturity;
- browser-wide benchmark superiority;
- battery-life superiority;
- general public readiness.
