# Altru Browser Developer Preview

The Linux developer preview is the first interactive shell around the Altru-owned native engine.

It is intentionally a **bounded browser preview**, not a production browser.

## What works

- native desktop window;
- Altru Browser application identity and icon;
- Focus / Navigate / Inspect interface states;
- Ctrl+L address focus;
- back, forward and reload through the owned BrowserKernel history;
- HTTPS top-level document loading through an explicit ResourceBroker;
- native HTML/CSS/layout execution;
- bounded N2.1 flex/grid geometry when the desktop preview feature is enabled;
- retained-scene rendering into the native shell;
- exact execution evidence visible through Inspect;
- light/dark shell toggle;
- no behavioral telemetry.

## Current network boundary

The preview permits only:

- the built-in `altru://start` document; or
- top-level `https://` documents.

It denies:

- plain HTTP;
- URLs containing embedded credentials;
- documents above 2 MiB;
- unsupported top-level media types;
- redirects in the preview broker.

External stylesheets, scripts, images, fonts and other subresources are not fetched yet.

## Fail-closed behavior

If a page requires markup, CSS, scripting or other behavior outside the native engine's current supported subset, Altru Browser shows the engine error instead of silently approximating the page or substituting Chromium/WebKit.

## Build

```bash
cargo build --release --features desktop-preview --bin altru-browser
```

Run:

```bash
target/release/altru-browser
```

## Linux user install

```bash
scripts/install-linux-preview.sh
```

This installs the exact locally built release binary under `~/.local/lib/altru-browser/`, the application icon under `~/.local/share/icons/`, and an `Altru Browser` desktop entry under `~/.local/share/applications/`.

## Interaction

- **Ctrl+L** — Navigate / focus address field
- **Esc** — Focus mode
- **F9** — Inspect mode

Focus is the default browsing principle: the page receives as much screen area as possible. Navigation and inspection surfaces appear in response to intent.

## Visual identity

The primary application identity is the **vyshyvanka flower**. Interface embroidery motifs are intentionally low-contrast and structural rather than decorative.

See `docs/BRAND.md`.

## Not yet implemented

- JavaScript;
- cookies/storage;
- external CSS and image loading;
- forms;
- downloads;
- tabs/workspaces;
- browser sandbox/site isolation;
- production security;
- full HTML/CSS/Web API conformance.

Those remain explicit development milestones.
