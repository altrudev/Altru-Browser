# EDRG — native root-font-relative resolution (2026-10-13)

## Authority
Development performed on isolated Prometheus Altru-Browser worktree. Native rendering required; Servo remains optional and disabled. No server deployment or browser-profile/credential access authorized.

## Native implementation
- Added a dedicated root-font-size basis to the CSS length IR, distinguished from parent/current em.
- Added bounded compiler translation `css.length-rem.root-font.v1`; source is SHA-256-bound in the verified translation registry.
- Computed styles propagate the resolved `<html>` font size through descendant elements. When the root font changes, a fresh style calculation derives updated rem geometry.
- Direct lengths and CSS custom-property substitutions use root-aware resolution. Unsupported values are not invented and resource access stays brokered.

## EDRG evidence
- 205 default library tests passed; zero failed.
- Full `scripts/verify-edge-compatibility.sh` passed offline, including Taffy and differential fixture verification.
- Root-font regression: 20px root plus 2.25rem yields 45px, 24px root yields 54px; descendant parent font can differ.
- Custom-property regression: `--space: 2.25rem` resolves against a 24px root to 54px.
- Live CNET native test executed explicitly; next observed failure is `UnresolvedCustomProperty("--spacing-15")`. It is **not** a release pass.
- Strict clippy/rustfmt repository gates had pre-existing failures in preceding sprint and were not re-certified here.
- No independent signed Frequency gateway receipt, external reference-browser frame comparison, or remote-GUI acceptance exists for this sprint.

## Release disposition
BLOCKED. These are real native-rendering improvements but do not establish broad browser compatibility. Next architecture requirement is a complete CSS custom-property cascade/dependency resolution model, with CSS invalid-at-computed-value-time handling and independent rendering comparisons. Do not assign fallback or user permissions to work around missing CSS semantics.
