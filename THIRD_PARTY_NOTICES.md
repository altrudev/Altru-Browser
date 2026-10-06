# Third-Party Notices

Altru Browser depends on third-party software. Those dependencies remain governed by their own licenses; the Altru Browser MIT/Apache-2.0 choice does not relicense them.

## Direct dependencies

| Package | Version | License | Role |
|---|---:|---|---|
| `hex` | 0.4.3 | MIT OR Apache-2.0 | hexadecimal encoding |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | SHA-256 evidence hashing |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT | optional Servo adapter/windowing types |
| `url` | 2.5.8 | MIT OR Apache-2.0 | optional Servo adapter URL handling |
| `servo` | 0.6.0 | MPL-2.0 | optional compatibility/reference engine |
| `taffy` | 0.14.0 | MIT | optional layout-geometry experiment |

Servo is an optional dependency and remains separately licensed under MPL-2.0. Altru Browser does not relicense Servo source as MIT/Apache-2.0 and does not copy Servo source into the Altru-owned native implementation.

Taffy is optional and remains behind the Altru-owned layout semantics boundary.

## Transitive dependencies

The all-features dependency review recorded **732 packages**, with Cargo license metadata present for every package in the resolved graph at the time of review. See `docs/DEPENDENCY-REVIEW.md` and `Cargo.lock` for the reproducible dependency set.

This file is an attribution and licensing aid, not a substitute for the license files distributed with third-party packages.
