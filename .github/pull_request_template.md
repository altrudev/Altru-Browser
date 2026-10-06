## Summary

Describe the behavior being changed and why.

## Boundary affected

Which Altru Browser semantic, security, platform, resource, or evidence boundary changes?

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo test --release`
- [ ] relevant feature/platform tests
- [ ] negative/failure cases added where applicable

## Evidence and limits

Describe what proves the change and what remains unsupported.

## Dependencies / authority

- [ ] no unnecessary dependency added
- [ ] no third-party type became semantic authority
- [ ] no telemetry, credential, customer-data, or private Frequency dependency introduced
