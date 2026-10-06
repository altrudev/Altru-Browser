# Native Engine DDC/Frequency Gate

## Purpose

Grow AWEF from a bounded Light Plane into an independently owned, cross-platform web engine without allowing a bootstrap dependency to become permanent architecture.

## Current decision

- AWEF-owned DOM model: **ALLOW / EXPERIMENT**
- AWEF-owned scene graph: **ALLOW / EXPERIMENT**
- deterministic test renderer: **ALLOW**
- cross-platform PlatformHost contract: **ALLOW**
- ScriptEngine contract: **ALLOW**
- concrete JavaScript runtime: **SIMULATE_FIRST**
- wgpu backend: **BENCHMARK_FIRST**
- Vello renderer: **BENCHMARK_FIRST**
- Parley/Fontique text stack: **BENCHMARK_FIRST**
- AccessKit adapters: **BENCHMARK_FIRST**
- Servo production dependency: **TEMPORARY / DEMOTION TARGET**
- Servo differential oracle: **ALLOW**
- platform-specific APIs in native core: **BLOCK**

## N1 promotion requirements

The Native Document Engine cannot advance beyond EXPERIMENT until all of the following are true:

1. it compiles and tests with the `servo-engine` feature disabled;
2. its document model and mutation epochs are deterministic;
3. malformed/unsupported syntax fails closed;
4. output is generated from an AWEF-owned scene representation;
5. no platform-specific crate is imported by the semantic core;
6. every external primitive is behind an AWEF-owned interface;
7. conformance fixtures are content-addressed;
8. native-vs-oracle differences are recorded, not silently normalized;
9. production promotion remains false.

## Platform invariant

The semantic engine must compile without selecting a concrete OS.

Allowed:

```text
AWEF core -> PlatformHost trait -> platform adapter
```

Blocked:

```text
AWEF DOM/layout -> Win32/X11/Wayland/AppKit/UIKit/Android API
```

## Servo independence metric

Track a monotonically increasing native-coverage ledger:

```text
capability
native status
standards tests passed
oracle comparison count
known divergences
supported platforms
promotion state
```

Servo removal is a measurable threshold decision, not a rewrite event.

## Dependency rule

A third-party primitive is admitted only when:

- its role is below an AWEF-owned semantic contract;
- its license/provenance is acceptable;
- there is a replacement boundary;
- its failure cannot manufacture stronger compatibility or authority claims;
- it is benchmarked on at least Linux and one non-Linux target before platform promotion.

## Immediate N1 scope

Implement only:

- document tree;
- mutation model;
- text nodes;
- block/inline scene primitives;
- deterministic scene serialization;
- platform-neutral renderer boundary;
- capability manifest;
- provenance/promotion metadata.

Do not add JavaScript, CSS Grid, media, WebGL, WebRTC or service workers merely to increase apparent feature count.
