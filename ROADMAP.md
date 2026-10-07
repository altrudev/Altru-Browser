# Altru Browser Roadmap

This roadmap describes capability progression. Dates are intentionally not promised; promotion depends on reproducible evidence.

## Current — Native foundation

- owned DOM and HTML parser;
- bounded CSS parser and cascade;
- style inheritance and custom properties;
- invalidation;
- block-oriented layout;
- retained scene representation;
- deterministic evidence receipts;
- explicit resource broker;
- cross-platform PlatformHost contract;
- Linux runtime verification;
- Windows/macOS/Android/iOS compile verification.

## N2.1 — Bounded flex/grid semantics (experimental)

Implemented on the N2.1 development line:

- owned `display:flex` and `display:grid` computed semantics;
- owned `flex-direction`, pixel `gap`, and bounded equal-`1fr` grid column semantics;
- neutral Altru geometry contract between layout semantics and adapters;
- optional Taffy translation beneath that contract;
- explicit fail-closed behavior when flex/grid requires an unavailable adapter;
- positive, negative, equivalence, and deterministic geometry tests.

This is **not** full Flexbox/Grid conformance. Standards-derived/WPT expansion remains a promotion gate.

## N3 — Scripting foundation

- introduce the first ScriptEngine implementation;
- keep host objects and Web APIs Altru-owned;
- define task/microtask scheduling;
- integrate Test262-derived conformance evidence;
- maintain explicit authority boundaries.

## N4 — Web APIs and browser services

- URL/fetch/networking;
- storage;
- timers;
- events;
- forms;
- history/navigation;
- accessibility;
- permissions and origin/security controls.

## N5 — Rendering and text

- production renderer candidate behind retained scene;
- GPU/CPU fallback strategy;
- font discovery and shaping;
- damage tracking;
- accessibility tree adapters;
- measurable resource governance.

## N6 — Native default

Altru Native becomes the default only after defined compatibility, security, stability and performance thresholds are met. Servo moves to differential/reference testing and can ultimately be removed from production dependencies.

## Browser shell

The user shell will evolve in parallel with the engine:

- top URL bar remains a first-class navigation primitive;
- workspaces/tabs;
- desktop and mobile layouts;
- privacy controls;
- resource visibility;
- subtle Ukrainian-inspired visual language;
- accessibility and keyboard navigation from the beginning.

The shell must never claim capabilities the engine has not earned.
