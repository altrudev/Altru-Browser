# Altru Session

Altru Session is the display layer of the Altru platform: a Wayland compositor in which
native work tools run at full speed next to Altru apps. It's not a browser shell. The
first goals are music production, photo and art, and development.

```bash
cargo build -p altru-session
altru-session --profile studio            # Ardour, qpwgraph, terminal
altru-session --profile art               # Krita, darktable, GIMP
altru-session --profile dev --run "foot"  # VS Code, terminal, plus anything else
```

`altru-session` is a workspace member but not a default member, so root `cargo build`,
`cargo test` and the audit scripts still cover only the engine.

## What works (v0.1)

- **Nested backend (winit):** runs as a window inside an existing desktop.
- **Automatic layout:** the first app listed is the main window on the left, the rest
  stack on the right. No overlapping, no dragging. Apps are told they are tiled.
  Launch order is kept however fast each app starts (matched by process id).
- **Profiles:** missing tools are reported with the package to install.
- **Apps stay in the session:** Wayland toolkits are selected and `DISPLAY` is removed,
  so nothing silently opens on another desktop.
- **Demand-driven drawing:** redraws only when something changed, checked once per
  60 Hz frame. Measured idle CPU on the VPS (software rendering): about 1%.

Tested on the VPS under Xvfb with three `foot` terminals; 9 unit tests cover the layout
and the command-line parsing.

## Known limits

- **No XWayland yet:** X11-only programs (for example some DAWs and plugin UIs) won't
  open. Ardour, Krita, darktable, GIMP 3 and VS Code have Wayland support.
- No keyboard shortcuts to switch focus or change the main window yet.
- Runs inside a desktop only. Running directly on the hardware is next.

## Next

1. **DRM/KMS + libinput backend:** boot straight into Altru Session on the G700
   (and real workstations), with GPU rendering.
2. **XWayland:** X11 apps and plugin windows.
3. **Audio:** a low-latency PipeWire profile (quantum and sample rate per profile)
   and per-app audio permissions.
4. **Keyboard:** switch windows, promote a window to main, open profiles.
5. **Altru apps:** Altru Browser / PWX apps as windows of the session.

The compositor core started from Smithay 0.7.0's `smallvil` example (MIT,
`session/LICENSE-SMITHAY`).
