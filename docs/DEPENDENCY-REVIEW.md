# Dependency and License Review

Generated from `cargo metadata --all-features` for the Altru Browser community-release candidate.

## Direct dependencies

| Crate | Version | Declared license | Repository |
|---|---:|---|---|
| `dpi` | `0.1.2` | `Apache-2.0 AND MIT` | https://github.com/rust-windowing/winit |
| `hex` | `0.4.3` | `MIT OR Apache-2.0` | https://github.com/KokaKiwi/rust-hex |
| `servo` | `0.6.0` | `MPL-2.0` | https://github.com/servo/servo |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` | https://github.com/RustCrypto/hashes |
| `taffy` | `0.14.0` | `MIT` | https://github.com/DioxusLabs/taffy |
| `url` | `2.5.8` | `MIT OR Apache-2.0` | https://github.com/servo/rust-url |

## Transitive metadata check

- Total packages in all-features graph: **732**.
- Packages with no declared Cargo license metadata: **0**.
- Every package in the resolved all-features graph declares license metadata.

## Boundary

Servo is optional and remains a compatibility/reference path. Its dependency graph is not copied into Altru-owned source. Taffy is optional and used only behind the owned geometry boundary.

This inventory is evidence for dependency review; it is not a legal opinion.
