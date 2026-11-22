# Altru Session

Altru Session is the display layer of the Altru platform: a Wayland compositor in which
native work tools run at full speed next to Altru apps. It's not a browser shell. The
first goals are music production, photo and art, and development.

```bash
cargo build --release -p altru-session
altru-session --profile studio            # Ardour, qpwgraph, terminal; low-latency audio
altru-session --profile art               # Krita, darktable, GIMP
altru-session --profile dev --run "foot"  # VS Code, terminal, plus anything else
```

`altru-session` is a workspace member but not a default member, so root `cargo build`,
`cargo test` and the audit scripts still cover only the engine.

## What it does (v0.2)

| Area | Status |
| --- | --- |
| **Runs on the hardware** (DRM/KMS + libinput + libseat) | built, compile-verified; **needs the G700 test** |
| **Runs inside a desktop** (winit window) | tested |
| **Automatic layout:** first app is the main window, others stacked; launch order kept | tested |
| **Keyboard control** (table below) | tested |
| **XWayland:** X11-only apps tiled like the rest | tested (xclock, xcalc) |
| **Low-latency audio** for the studio profile (PipeWire) | tested |
| **GPU apps** (linux-dmabuf, hardware backend) | built; needs the G700 test |
| **Demand-driven drawing:** ~1% CPU when idle | tested (nested) |

### Backends

- `--backend auto` (default): inside a desktop (`DISPLAY` or `WAYLAND_DISPLAY` set) it
  opens as a window; on a text console it takes over the machine.
- `--backend drm`: the hardware.
  - **Session:** libseat gives GPU and input access without root.
  - **Screens:** every connected screen, left to right, at its preferred mode.
  - **Drawing:** OpenGL ES composition on GBM buffers, page flips on vblank, direct
    scanout where possible.
  - **Input:** libinput (keyboards, mice, touchpads, tablets).
  - **GPU apps:** linux-dmabuf.
  - **Limits:** one GPU (the boot GPU); simple built-in cursor when an app doesn't set one.
- `--backend winit`: a window inside the current desktop, for development.

### Keyboard

Everything uses the Super (Windows) key, so app shortcuts pass through untouched.

| Keys | Does |
| --- | --- |
| Super + Tab / Super + Shift + Tab | next / previous window |
| Super + 1 … 9 | go to window 1 … 9 (1 is the main one) |
| Super + Space | make this window the main one |
| Super + Enter | open a terminal |
| Super + Q | close this window (the app may ask first) |
| Ctrl + Alt + F1 … F12 | switch to a text console (hardware backend) |
| Ctrl + Alt + Backspace | leave Altru Session |

### Audio

`--profile studio` sets PipeWire to 128 frames at 48 kHz (about 2.7 ms) while the
session runs, gives apps the same setting (`PIPEWIRE_LATENCY`), and puts PipeWire's
defaults back when the session ends. `--latency low|default|FRAMES[@RATE]` overrides,
for any profile (for example `--latency 64@96000`).

If the session is killed instead of closed with Ctrl+Alt+Backspace, the setting stays
until PipeWire restarts or `pw-metadata -n settings 0 clock.force-quantum 0` is run.

### XWayland

Starts with the session. Profile apps start once it's ready, so X11-only programs find
a display. X11 apps get `DISPLAY` pointing at this session's XWayland, never at
another desktop. If XWayland isn't installed, the session says so and runs
Wayland-only.

## Known limits

- **No clipboard between X11 and Wayland apps yet.** Copy and paste works within each side.
- No primary selection, fractional scaling, server-side decorations or screen capture yet.
- **One GPU.** Hybrid-graphics laptops use the boot GPU.
- No display settings yet (resolution, scale, arrangement): preferred mode, left to right.

## G700 test plan

Do this once, at the laptop itself (not over SSH).

1. **Install** a minimal Ubuntu or Debian (no desktop). Then:
   ```bash
   sudo apt install -y build-essential pkg-config git curl libwayland-dev libudev-dev \
     libinput-dev libgbm-dev libseat-dev libxkbcommon-dev libdrm-dev libegl1-mesa-dev \
     libgles-dev xwayland pipewire pipewire-bin wireplumber foot seatd
   curl https://sh.rustup.rs -sSf | sh
   git clone -b altru-metal https://github.com/altrudev/Altru-Browser && cd Altru-Browser
   cargo build --release -p altru-session
   ```
   On a system without logind, also enable `seatd` and add yourself to the `seat`
   group: `sudo systemctl enable --now seatd && sudo usermod -aG seat $USER`.
2. **Log in on the text console** and run `./target/release/altru-session --run foot`.
   - Expected: the screen switches to the session and a terminal fills it.
   - If it fails, it prints why (for example "no seat"). Send that line.
3. **Mouse and keyboard:** move the pointer, click into the terminal, type `ls`.
4. **Layout and shortcuts:** Super+Enter twice (three terminals); Super+2 then
   Super+Space (window 2 becomes main); Super+Q closes it.
5. **Console switch:** Ctrl+Alt+F2 goes to a text console; Ctrl+Alt+F1 (or the VT you
   started on) comes back with everything intact.
6. **X11 app:** `--run xclock` (from `x11-apps`) appears tiled.
7. **GPU app:** in the terminal, `glxgears` or `es2gears_wayland` (`mesa-utils` /
   `mesa-utils-extra`) should run smoothly. Then open Krita or darktable.
8. **Audio:** `--profile studio --run foot`, then
   `pw-metadata -n settings 0 | grep force` shows `128` and `48000`.
9. **Leave** with Ctrl+Alt+Backspace: you're back at the console, and the audio setting
   is reset.

Send a phone photo of the screen at steps 2, 4 and 7, plus the terminal output if
anything fails.

The compositor core started from Smithay 0.7.0's `smallvil` example (MIT,
`session/LICENSE-SMITHAY`).
