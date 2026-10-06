# Public / Private Boundary

Altru Browser is the public community repository for the independently owned browser and native-engine work.

The native engine retains the engineering codename **Adaptive Web Engine Fabric (AWEF)**. Private Altru.dev development-assurance systems are deliberately outside this repository.

## Public here

The community repository may contain:

- browser/navigation kernel;
- native DOM and HTML implementation;
- CSS parsing, cascade and invalidation;
- layout contracts and scene semantics;
- renderer interfaces and neutral adapters;
- PlatformHost contracts;
- ScriptEngine interfaces and public implementations;
- resource/network authority interfaces;
- deterministic evidence schemas;
- tests, fixtures, benchmarks and conformance tooling;
- reproducible release/audit scripts;
- public documentation.

## Not public here

This repository must not contain:

- private Frequency reasoning/evolution implementation;
- private policy graphs or learning state;
- credentials or credential-broker internals;
- customer evidence or customer data;
- proprietary service integrations;
- deployment secrets;
- private operational infrastructure.

Public Altru Browser code must build and function without the private assurance layer.

## Licensing

Unless a file states otherwise, Altru Browser source is dual-licensed under **MIT OR Apache-2.0**.

Brand names, logos and service marks are not granted by the source-code license.

## Clean-room publication rule

Changes originating in private development environments may enter this repository only when:

1. the public implementation is independently reviewable;
2. no private code, credentials or customer data is included;
3. licensing and attribution are compatible;
4. tests/evidence accompany substantive behavior changes;
5. the community-release audit passes.

The public Git history is intentionally independent from private experimental history.
