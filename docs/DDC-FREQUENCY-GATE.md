# DDC / Frequency Gate — Phase 0

## Purpose

Determine whether an adaptive multi-plane web runtime can reduce resource cost without manufacturing compatibility claims.

## Authority envelope

Allowed in Phase 0:
- parse local test HTML;
- classify capability signals;
- route to an in-process light stub or Servo adapter boundary;
- measure local process behaviour;
- emit deterministic receipts.

Not allowed in Phase 0:
- patch Servo;
- claim general web compatibility;
- silently execute uncertain content in the light plane;
- migrate a live JavaScript heap between engines;
- access credentials or user browser profiles;
- perform network side effects from test documents.

## Decisions

- Light engine prototype: SIMULATE_FIRST
- Servo adapter boundary: ALLOW
- Fail-closed classifier: ALLOW
- Continuity capsule implementation: SIMULATE_FIRST
- Cross-engine live-state migration: BLOCK
- QuickJS/Boa: BENCHMARK_ONLY until isolated capability contracts exist
- Zero-copy IPC: BENCHMARK_ONLY
- CRIU lazy restoration: EXPERIMENTAL_LINUX_ONLY

## Promotion evidence

A candidate may be promoted only if:
1. semantic test corpus passes;
2. fallback is deterministic;
3. no weaker authority is introduced;
4. measured RSS/CPU/latency improves against baseline;
5. failure restores to Servo or a safe terminal state;
6. evidence is reproducible on at least two environments before superiority claims.

## Current claim

Phase 0 verifies only the routing/evidence architecture. It does not verify browser performance or compatibility.
