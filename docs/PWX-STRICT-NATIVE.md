# PWX Strict on the native engine

A PWX Strict app never touches a document. Its code runs in a sealed worker and sends
a short list of UI operations: `create`, `attr`, `prop`, `text`, `append`, `remove`,
`focus`, `listen`. The PWX runtime applies only allowlisted tags, attributes,
properties and events.

`src/pwx_strict.rs` is that renderer, owned by Altru Browser. The operations build an
Altru-owned tree, which becomes a `NativeDocument` for the native style, layout and
scene pipeline, and from there the Altru Metal screen. Altru's own apps then need no
general web compatibility: only this small, fixed interface.

```text
PWX app code (sealed worker)
      | UI operations
StrictBridge (allowlists = PWX renderer.js)
      | snapshot per frame
NativeDocument -> native CSS -> layout -> scene -> Altru Metal
```

## Status

Experimental. Nothing here is promoted.

- **Allowlists** match PWX spike 0.4.10 `app-origin/renderer.js` exactly. Rejections
  are recorded in the same `op:reason:detail` form as PWX's `__pwxRejected`.
- **Stricter than PWX in two places:** node and text quotas fail closed, and `img src`
  is rejected because the native host has no blob store yet.
- **Form fields** show their value, or their placeholder when empty, as text. Real
  native form controls come later.
- **Rebuilds per frame.** In-place mutation through a DOM transaction and mutation
  journal is the next step. It needs detach support in `native_dom`, and every pass
  that walks the whole node arena must then skip detached nodes.
- **No JavaScript engine yet.** Until a `ScriptEngine` (Boa) runs the app,
  recorded operation streams drive the bridge.

## The shared stylesheet

`native_ui_stylesheet` loads PWX `ui.css` into the native CSS engine. Exactly two
constructs aren't native yet, and both are handled explicitly rather than approximated:

| `ui.css` construct | Native handling |
| --- | --- |
| `* { box-sizing: border-box; }` | dropped and reported (universal selector unsupported) |
| `@media (prefers-color-scheme: dark)` | the host knows its scheme: dropped for Light, its body applied for Dark |

Anything else the parser rejects is an error. `ui.css` lays the app out with flexbox,
so rendering needs `--features taffy-layout`. Without it, layout fails closed with
`UnsupportedLayout` (tested).

## Recording an app

```bash
node tools/pwx-strict-record.mjs <pwx>/spike/app-origin/renderer.js \
     <pwx>/spike/apps/notes/main.js --storage --seed fixtures/pwx-strict/notes-seed.json \
     > fixtures/pwx-strict/notes-seeded.strict
```

The recorder reads the real `PwxUI` prelude out of PWX's `renderer.js`, so there is one
source of truth, and runs the app in a Node `vm` context with no DOM and no network.
`--click "Label"` and `--type "Field=text"` script user actions.

Fixtures (recorded from PWX `spike` at `0f8ebf5`; `ui.css` copied from the same commit):

| Fixture | What it shows |
| --- | --- |
| `notes-empty.strict` | first run, empty state |
| `notes-seeded.strict` | two saved notes, newest opened |
| `notes-new-typed.strict` | New note clicked, title and body typed, saved |

## Probe

```bash
# Tree + rejections, and the same tree as HTML for comparison in Chromium/Firefox/WebKit:
cargo run --bin strict_probe -- fixtures/pwx-strict/notes-seeded.strict \
    fixtures/pwx-strict/ui.css --html notes.html

# Native layout, drawn by Altru Metal into a picture, or full-screen on the G700:
cargo run --features taffy-layout --bin strict_probe -- \
    fixtures/pwx-strict/notes-seeded.strict fixtures/pwx-strict/ui.css --ppm notes.ppm
sudo ./target/debug/strict_probe fixtures/pwx-strict/notes-seeded.strict \
    fixtures/pwx-strict/ui.css --screen
```

The HTML export is the differential reference: the same tree in Chromium should look
like the PWX Notes app, and the native rendering is measured against it.

## Next

1. Run `notes_render_through_native_style_and_layout` with `taffy-layout` and fix
   what native layout gets wrong against the Chromium reference.
2. Boa behind `ScriptEngine` runs the app's code with `PwxUI` as its only host object.
3. Hit testing from layout fragments back to app node ids (`native_for`), so clicks
   and typing on Altru Metal reach the app.
4. The storage capability backed by Altru Metal's per-app vault.
