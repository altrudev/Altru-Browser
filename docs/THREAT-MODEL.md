# Threat Model

This document defines the initial security boundary for the experimental Altru Browser engine.

## Assets

- user browsing data;
- credentials and secrets;
- local files;
- origin-isolated storage;
- process and device resources;
- navigation intent;
- execution evidence.

## Major threat classes

- malicious web content;
- origin confusion;
- script or Web API authority escalation;
- renderer/parser memory-safety defects;
- dependency compromise;
- malicious extensions or future plugins;
- unsafe platform integration;
- telemetry or privacy leakage;
- stale or forged execution evidence.

## Design responses

- Rust-first memory-safe core where practical;
- explicit resource broker rather than ambient authority;
- PlatformHost boundary for OS capabilities;
- fail-closed unsupported behavior;
- deterministic evidence for promoted paths;
- minimized dependencies;
- private credentials excluded from repository and runtime contracts;
- security maturity tracked separately from feature completeness.

## Out of scope today

The current experimental engine does not yet claim a complete production sandbox, hardened multi-process site isolation, mature extension security model, or full exploit-resistance parity with established browsers.

Those remain release blockers for production use.
