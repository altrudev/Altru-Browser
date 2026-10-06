# Altru Browser Architecture

Altru Browser is an independent browser project. The native engine is developed under the engineering codename **Adaptive Web Engine Fabric (AWEF)**.

The design goal is not to wrap Chromium or permanently depend on another browser engine. Altru owns the browser semantics and may use replaceable libraries only beneath explicit boundaries.

## High-level model

```text
Altru Browser shell
      |
Browser / Navigation Kernel
      |
Native Document Runtime
  +-- DOM + HTML
  +-- CSS + cascade
  +-- invalidation
  +-- layout contracts
  +-- retained scene
  +-- ScriptEngine boundary
      |
Renderer abstraction
      |
PlatformHost
  +-- Linux
  +-- Windows
  +-- macOS
  +-- Android
  +-- iOS
```

## Ownership boundary

Altru Browser owns behavior that defines web semantics or user trust:

- DOM and mutation semantics;
- HTML parsing policy;
- CSS parsing, cascade, inheritance and invalidation;
- layout semantics and formatting-context selection;
- navigation/history;
- origin/security policy;
- storage policy;
- task and microtask scheduling;
- Web API registration;
- accessibility semantics;
- execution evidence and promotion state.

Replaceable primitives may provide geometry, rendering, text shaping, JavaScript execution, networking, or platform adaptation, but they must remain behind Altru-owned interfaces.

## Current state

The current native engine is experimental.

Implemented foundations include owned DOM/HTML, bounded CSS/cascade, invalidation, block-oriented layout, retained scene generation, deterministic evidence, resource authority boundaries and cross-platform host contracts.

Servo 0.6.0 remains optional as a compatibility/reference path during migration. Taffy remains an optional geometry candidate. Neither defines the long-term identity of Altru Browser.

See `docs/NATIVE-ENGINE-ROADMAP.md` for the detailed migration plan.
