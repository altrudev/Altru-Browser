# Altru Metal

Altru Metal is an experimental `PlatformHost` in which Altru Browser **is** the screen.
It does not open a window inside another desktop. It draws directly to the Linux
framebuffer and reads the keyboard directly from the kernel. Linux is used only as
the driver substrate, the same position DDC-OS takes.

The aim is a private web computer: the machine boots straight into Altru Browser,
apps are signed PWX packages stored on the disk, each app gets its own private
storage, and the network is off for each app unless the owner allows it.

## Status

Experimental first sketch. Nothing here is promoted.

| Provider | Altru Metal v0 |
| --- | --- |
| Window | whole screen, size read from the framebuffer |
| Graphics | Linux fbdev `/dev/fb0`, 32 bits per pixel only; anything else fails closed |
| Text | placeholder glyph cells at the real layout positions; a rasterizer comes next |
| Input | raw keyboard events from `/dev/input/event*` |
| Storage | one private vault per PWX app id, `<root>/apps/<id>/data`, mode 0700 |
| Network | denied by default; allowed per app and per origin |
| Secrets | PWX signer slot below the browser; no key material is reachable from pages |
| Clipboard, accessibility, media | not available yet |

Shell keys map to the approved interface states: F6 or Ctrl+L = Navigate,
F12 = Inspect, Esc = Focus, Ctrl+Q = quit.

## Try it

Without touching the screen (writes a picture you can open anywhere):

```bash
cargo run --release --bin metal_probe -- fixtures/native-n1/headings.html --ppm metal.ppm
```

On the target machine, from a text console with no desktop running
(Ctrl+Alt+F3, or a minimal install without a desktop):

```bash
cargo build --release --bin metal_probe
sudo ./target/release/metal_probe fixtures/native-n1/headings.html
```

`sudo` is needed only because `/dev/fb0` and `/dev/input` are root-owned by default.
Adding the user to the `video` and `input` groups removes that need.

## Reference hardware

First runtime target: Lenovo G700 (Intel 3rd generation, HD 4000 graphics,
1600x900 panel). The i915 driver exposes a 32-bit framebuffer at `/dev/fb0`.

## Known limits

- Keystrokes also reach the text console underneath. A later version takes the
  console over in graphics mode.
- fbdev is a legacy interface. The planned replacement is DRM/KMS with
  double buffering, which needs `ioctl` support and therefore a `libc` dependency.
- The native engine has no JavaScript yet (milestone N3). PWX apps need it, so
  running real apps on Altru Metal waits on N3 or uses the Servo adapter.

## Runtime evidence required before promotion

Per `docs/PLATFORM-MATRIX.md`: engine version, OS and architecture, fixture manifest
hash, execution hash, host identity (`altru-metal-fbdev-v0`), renderer identity
(`altru-metal-cpu-v0`), and crash/failure result, captured on real hardware.
