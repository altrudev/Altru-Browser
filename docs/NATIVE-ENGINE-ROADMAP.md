# Native Engine Independence Roadmap

## Goal

AWEF must be able to become a complete browser engine without requiring Servo at runtime.

Servo is a temporary compatibility oracle and bootstrap implementation. It is not part of the long-term identity of the browser.

The target is not "no third-party libraries." The target is:

- no Servo runtime dependency;
- no Servo-owned browser semantics;
- AWEF-owned DOM, style, layout, navigation, Web API, scheduling and security contracts;
- portable platform boundaries from the first implementation;
- standards-driven conformance measured against WPT and Test262 rather than against Servo internals;
- replaceable implementation modules behind AWEF-owned interfaces.

## Platform targets

Tier 1:
- Linux
- Windows
- macOS

Tier 2:
- Android

Tier 3:
- iOS / iPadOS where alternative-engine distribution is permitted by platform policy and entitlement.

The browser core must never contain Linux-only assumptions.

## Architectural boundary

```text
                         AWEF BROWSER

                 Browser / Navigation Kernel
                           |
                     Document Runtime
                           |
          +----------------+----------------+
          |                |                |
        DOM/Core       Style/Layout       Script
          |                |                |
          +----------------+----------------+
                           |
                       Scene Graph
                           |
                 Renderer Abstraction
                           |
        +------------------+------------------+
        |                  |                  |
      WGPU             CPU renderer       test renderer
        |
   DX12 / Vulkan / Metal / GLES

                 PlatformHost boundary
        +----------+----------+----------+
        |          |          |          |
     Windows     macOS      Linux      Mobile
```

Servo is not inside this architecture. During migration it lives beside it as a differential oracle:

```text
fixture
  |--------------------|
  v                    v
AWEF Native          Servo
  |                    |
  +------ compare ------+
          |
      evidence
```

## Owned semantic layers

AWEF should own these layers because they define browser behavior:

1. Document tree and lifecycle.
2. DOM mutation semantics.
3. HTML token-to-tree integration.
4. CSS cascade and computed-style model.
5. Selector matching.
6. Layout tree and invalidation.
7. Fragmentation and scrolling.
8. Display-list / scene generation.
9. Event dispatch.
10. Navigation/history.
11. origin/site/security model.
12. cookies/storage policy.
13. Web API capability registry.
14. task/microtask scheduling.
15. page lifecycle and suspension.
16. resource prioritization.
17. accessibility tree semantics.
18. execution receipts and provenance.

Primitive libraries may be composed below these contracts when they do not own browser semantics.

## Primitive candidates

### Graphics

Preferred abstraction: `wgpu`.

Backends:
- Windows: DX12 / Vulkan.
- Linux: Vulkan, with GLES fallback where useful.
- macOS/iOS: Metal.
- Android: Vulkan / GLES.

Renderer candidates:
- Vello CPU for deterministic/software rendering.
- Vello GPU or direct wgpu scene renderer for accelerated paths.

AWEF owns the scene graph and damage/invalidation semantics.

### Text

Preferred initial stack:
- Parley for text layout;
- Fontique for font enumeration/fallback;
- HarfRust/Skrifa/ICU4X primitives.

This avoids making Linux Fontconfig a browser-core requirement.

Platform font adapters remain optional optimizations, not semantic dependencies.

### Accessibility

Use an AWEF accessibility tree with AccessKit as a platform adapter.

### JavaScript

Initial candidate: Boa behind `ScriptEngine`.

Requirements before promotion:
- Test262 tracking;
- deterministic execution controls;
- interrupt/budget support;
- task/microtask integration;
- host-object boundary controlled by AWEF;
- no ambient filesystem/network authority.

Boa is an implementation, not the AWEF API. A second JS engine can be benchmarked behind the same contract.

### HTML

Short-term parser implementations may be used only behind `HtmlParser`.

Long-term independence target:
- AWEF-owned token/tree semantics or a neutral parser whose behavior is pinned to the HTML specification and WPT;
- no final dependency on Servo runtime crates.

### CSS

Do not make Stylo the permanent AWEF style engine if strict Servo independence is the goal.

Create an AWEF-owned style contract:
- tokenization/parsing;
- selector compilation;
- cascade;
- inheritance;
- custom properties;
- computed values;
- invalidation.

Neutral parsing libraries may bootstrap pieces, but the browser semantic layer remains ours.

### Layout

Do not equate a generic layout library with web layout.

AWEF owns the web layout model. General-purpose primitives such as Taffy may be benchmarked for flex/grid calculations but must not become an unexamined compatibility authority.

## Cross-platform interfaces

The native engine may call OS services only through AWEF contracts:

```rust
trait PlatformHost {
    fn windowing(&self) -> &dyn WindowProvider;
    fn graphics(&self) -> &dyn GraphicsProvider;
    fn fonts(&self) -> &dyn FontProvider;
    fn clipboard(&self) -> &dyn ClipboardProvider;
    fn accessibility(&self) -> &dyn AccessibilityProvider;
    fn storage(&self) -> &dyn StorageProvider;
    fn networking(&self) -> &dyn NetworkProvider;
    fn media(&self) -> &dyn MediaProvider;
    fn secrets(&self) -> &dyn SecretProvider;
    fn sandbox(&self) -> &dyn SandboxProvider;
}
```

No core crate may import X11, Wayland, Win32, AppKit/UIKit, Android Java APIs, or platform filesystem conventions directly.

## Execution architecture

```text
HTML bytes
   |
Tokenizer / Tree Builder
   |
Document / DOM
   |
Style Resolver
   |
Layout
   |
Scene Builder
   |
Retained Scene
   |
Damage / Region Scheduler
   |
Renderer
   |
Platform Surface
```

JavaScript is attached through host bindings:

```text
ScriptEngine
   |
HostObject API
   |
DOM transaction
   |
mutation journal
   |
incremental style/layout invalidation
```

This prevents the JS implementation from owning DOM semantics.

## Native capability router

AWEF should keep the existing adaptive idea after Servo disappears.

```text
document/request
      |
capability classification
      |
+-----+----------------------+----------------+
|                            |                |
Static document          Interactive       Dormant
|                            |                |
minimal runtime          full AWEF          capsule
```

"Full AWEF" is still our own engine. The router selects fidelity/cost inside our runtime rather than switching to a foreign engine.

## Conformance strategy

Compatibility is standards-driven.

Primary external oracles:
- Web Platform Tests for browser/web-platform behavior;
- Test262 for ECMAScript;
- accessibility test suites;
- CSS-specific WPT subsets;
- URL/encoding/HTTP standard suites.

Servo/Chromium/WebKit/Firefox may be used for differential diagnosis, but a disagreement is not automatically proof that AWEF is wrong. The relevant specification and standards tests remain authoritative.

## Migration sequence

### N0 — Current

- Light Plane exists.
- Servo is compatibility implementation.
- receipts and routing are owned by AWEF.

### N1 — Native document engine

Implement:
- real DOM;
- HTML parser;
- style model;
- block/inline text;
- scene graph;
- Vello/wgpu output;
- no JavaScript.

Servo remains oracle/fallback in development only.

### N2 — CSS/layout expansion

Add:
- selectors;
- cascade/inheritance;
- custom properties;
- flex;
- grid;
- positioning;
- overflow/scrolling;
- media queries;
- incremental invalidation.

Begin running relevant WPT subsets continuously.

### N3 — Script runtime

Introduce `ScriptEngine` with Boa candidate.

Add:
- DOM bindings;
- events;
- task/microtask queues;
- timers;
- fetch boundary;
- mutation-driven invalidation.

Track Test262 and WPT separately.

### N4 — Browser platform APIs

Implement capability modules:
- Fetch;
- URL;
- storage;
- cookies;
- history;
- workers;
- canvas;
- images;
- forms;
- accessibility.

High-risk APIs remain opt-in and sandboxed.

### N5 — Native engine default

AWEF Native becomes the default engine when its defined compatibility threshold is met.

Servo moves to:
- differential oracle;
- compatibility lab;
- development-only adapter.

No end-user browsing silently falls back to Servo.

### N6 — Servo removal

Servo is removed from production dependencies once:
- required WPT target is reached;
- required Test262 target is reached;
- crash/security gates pass;
- platform parity gates pass;
- representative-site corpus passes;
- no critical capability requires the Servo adapter.

Servo may remain in a separate test repository solely as a differential oracle.

## Multi-OS delivery

### Windows

- winit/native host shell;
- wgpu DX12 primary;
- Windows UI Automation through AccessKit;
- Windows sandbox adapter;
- DPAPI/credential vault adapter.

### macOS

- native host integration;
- wgpu Metal;
- NSAccessibility through AccessKit;
- Keychain;
- App Sandbox/process isolation.

### Linux

- Wayland primary, X11 fallback;
- Vulkan primary;
- AT-SPI through AccessKit;
- Secret Service;
- Landlock/namespaces where available.

### Android

- Android lifecycle host;
- Vulkan/GLES;
- Android accessibility adapter;
- Android Keystore;
- process/service sandbox boundaries.

### iOS/iPadOS

Keep browser core portable, but distribution is a policy capability rather than an architectural assumption.

A first-party alternative engine must use Apple's BrowserEngineKit/entitlement model where permitted. The iOS host therefore lives behind its own adapter and does not shape the engine core.

## Frequency promotion model

Every module carries:

```text
implementation
version
license/provenance
supported capability set
conformance evidence
performance evidence
security evidence
platform matrix
promotion state
```

Possible states:

- EXPERIMENT
- SIMULATE_FIRST
- ORACLE_COMPARED
- CONFORMANCE_PARTIAL
- PLATFORM_VERIFIED
- PROMOTED
- DEMOTED

No dependency becomes permanent merely because it was useful during bootstrap.

## Independence rule

AWEF is considered Servo-independent only when:

1. no production binary links Servo or Servo-only runtime crates;
2. no production functionality silently dispatches to Servo;
3. AWEF browser semantics are defined by AWEF contracts + standards;
4. Servo can be removed from the workspace without changing the user-facing browser;
5. conformance evidence can be reproduced without Servo.

That is the target architecture.
