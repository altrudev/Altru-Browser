# Altru Capability Library — DDC/Frequency Seed v1

## Purpose

The Capability Library is the prebuilt semantic layer used by Altru Companion and the Capability Compiler.

The goal is not to claim support for every Web feature. The goal is to pre-identify known technology, bind it to authoritative definitions and tests, and convert high-confidence families into small ACIR translations before a live page encounters them.

Runtime rule:

`known syntax -> capability id -> verified translation -> ACIR -> native primitive`

Unknown or unverified semantics remain fail-closed.

## Data-fusion sources

The library should be generated from pinned, versioned inputs rather than manually maintained feature strings.

### 1. Web Features

Use Web Features IDs as the stable feature namespace. This gives Altru a common identifier for platform capabilities instead of inventing an independent taxonomy.

Use the Web Features mappings dataset to connect those IDs to WPT and other public ecosystem identifiers.

### 2. MDN Browser Compat Data

Use BCD as a machine-readable feature inventory and source of specification links, status, and feature hierarchy.

BCD is discovery metadata, not a semantic oracle. Browser support claims are never translated directly into Altru support claims.

### 3. Web Platform Tests

Use WPT as the primary executable semantic oracle.

A translation candidate can move from `candidate` to `verified` only when a bounded test corpus proves the intended semantics and negative tests prove fail-closed behavior.

### 4. WHATWG / standards-owned static datasets

Where a standard publishes normative/static machine-readable data, ingest it directly as a pinned source. Named HTML character references are the first obvious example.

## Promotion states

- `inventory` — known feature, no Altru translation yet.
- `candidate` — deterministic mapping appears possible.
- `implemented` — compiler/native path exists.
- `verified` — bounded positive + negative corpus passes.
- `promoted` — allowed in the runtime translation registry.
- `state-bound` — requires live browser/user/environment state.
- `native-bound` — requires a new native primitive.
- `authority-bound` — changes network/storage/script/device authority and cannot be hot-added.

## Live repair classes

- **L0 / syntax-normalization** — immediate, no replay.
- **L1 / semantic lowering** — immediate or targeted style/layout replay.
- **L2 / state binding** — live once explicit interaction/environment state exists.
- **L3 / native primitive** — sandbox, verify, then targeted replay or reload.
- **L4 / authority change** — versioned engine update/restart only.

## Prebuild strategy

Do not prebuild one rule per spelling when multiple spellings collapse into one semantic family.

Example:

`min-width`, `max-width`, `min-resolution`, `screen and (...)`, and reduced-motion conditions already collapse into ACIR environment predicates.

The same approach should be used for selectors, values, DOM state, URL/HTTP behavior, HTML parsing, accessibility semantics, and later bounded API surfaces.

## Highest-value families to prebuild now

The machine-readable seed is in `capability-library/seed-v1.json`.

Priority A contains deterministic families with strong standards/WPT coverage and little or no new authority.

Priority B contains deterministic families that first need one reusable native/state primitive.

Priority C contains capabilities that should be inventoried now but not hot-enabled because they change browser authority.

## Library build pipeline

1. Pin source versions/commits.
2. Import feature IDs and source metadata.
3. Normalize into Altru capability families.
4. Generate candidate translations only for bounded deterministic constructs.
5. Bind every candidate to positive and negative test corpus IDs.
6. Run differential/WPT verification in isolation.
7. Produce a signed/hash-bound translation manifest.
8. Promote only verified entries into the runtime registry.
9. Store locally; no telemetry and no live dependency on third-party services.
10. Update through explicit versioned library releases.

## DDC/Frequency invariants

- Translation is not authority.
- Feature popularity is not semantic proof.
- Browser compatibility metadata is not execution permission.
- No runtime download-and-execute path.
- No external AI/model dependency.
- No silent approximation.
- Unsupported semantics remain explicit.
- Every promoted translation has provenance, test evidence, version, and rollback identity.
