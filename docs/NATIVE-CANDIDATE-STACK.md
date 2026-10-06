# Native Engine Candidate Stack

This file records candidates only. Inclusion here does not promote a dependency.

## Rendering

### wgpu

Candidate role:
- portable GPU abstraction beneath AWEF-owned scene semantics.

Current researched release line:
- wgpu 30.x (as required by Vello 0.11.0).

Promotion gate:
- Linux software/GPU benchmark;
- Windows DX12 compile/runtime;
- macOS Metal compile/runtime;
- Android Vulkan compile/runtime;
- no browser semantics delegated to wgpu.

### Vello

Candidate role:
- CPU/GPU 2D renderer beneath the AWEF scene graph.

Current researched release:
- Vello 0.11.0.

License:
- MIT OR Apache-2.0 for the main project.

Promotion gate:
- deterministic reference rendering;
- retained-scene update benchmark;
- resource-pressure behavior;
- cross-platform artifact comparison.

## Layout primitive

### Taffy

Candidate role:
- geometry primitive behind AWEF-owned layout semantics.

Current researched release:
- Taffy 0.14.0.

Capabilities:
- CSS Block
- Flexbox
- Grid

Promotion gate:
- feature-gated only;
- AWEF remains authoritative for DOM, cascade, formatting-context selection and evidence;
- differential geometry fixtures;
- cross-platform compile matrix;
- no direct exposure of Taffy style types in public AWEF engine contracts.

## Text

### Parley / Fontique

Candidate role:
- text shaping/layout primitive beneath AWEF-owned CSS/layout semantics.

Current researched release:
- Parley 0.11.x.

License:
- MIT OR Apache-2.0 for the main libraries.

Parley currently composes Fontique, HarfRust, Skrifa and ICU4X.

Promotion gate:
- Unicode/bidi corpus;
- CSS inline-layout mapping;
- font fallback determinism;
- Linux/Windows/macOS/Android/iOS platform-font adapter evidence.

## JavaScript

### Boa

Candidate role:
- first ScriptEngine implementation behind AWEF-owned host bindings.

Current researched release:
- Boa 0.22.

Current reported Test262 conformance:
- 95.60%.

License:
- MIT OR Unlicense.

Promotion gate:
- independent Test262 run;
- deterministic interruption/budget controls;
- task/microtask integration;
- host-object authority boundary;
- no ambient filesystem/network capability;
- WPT integration for exposed Web APIs.

## Dependency rule

A candidate becomes a permanent dependency only after Frequency records:

- exact version;
- license/provenance;
- capability contract;
- conformance evidence;
- performance evidence;
- security evidence;
- platform matrix;
- replacement strategy.

No dependency is promoted simply because it is currently the easiest implementation.
