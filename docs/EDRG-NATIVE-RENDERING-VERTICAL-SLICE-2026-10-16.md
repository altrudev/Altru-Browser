# EDRG native rendering vertical slice — 2026-10-16

## Architecture and scope
Altru Browser remains a native Rust engine with governed resource acquisition; no Chromium, Servo, WebView or hidden renderer fallback is used. This sprint connects standards token intake, bounded CSS browser recovery, media capability evaluation, computed text/background color, native layout image commands, HTTPS image decoding and egui texture painting. This is a functional vertical slice, not browser-wide standards compliance.

## Live CNET evidence on Prometheus
- Linked stylesheet discovery: 15 URLs (including third-party CDN), 417,584 bytes collected independently for vocabulary inventory; 532 media blocks contained 23 distinct query strings in sampled external CSS.
- Explicit ignored live CNET native document plus external CSS plus images test: **PASS**.
- Scene emitted **79** image layout commands from HTML; bounded resource scheduler fetched first **12** unique image resources; **12 decoded successfully, 0 failed**, after adding GIF and safely sizing 1920x1080 JPEG decoding.
- Browser-recovery CSS counted **174 unsupported selector lists** and **36 unsupported declarations** omitted, not claimed as supported. CSS media hover, pointer, color scheme and reduced motion predicates now use explicit environment predicates. CSS browser-mode recovery preserves supported adjacent rules; the strict DDC CSS compiler remains fail-closed.
- Native color-to-scene tests verify filled element backgrounds and inherited text colors. Native image fixture verifies HTTPS broker -> decoded pixels -> source-associated scene image geometry.

## Gates
- Default library unit tests: **220 passed**.
- Desktop-preview library tests: **232 passed**.
- Desktop-preview binary tests: **5 passed, 4 ignored live tests** by default.
- Full offline compatibility script: **EDGE_COMPATIBILITY_OFFLINE_PASS**.
- Live CNET full resources test: **1 passed** (run explicitly as ignored).
- HTTPS denial test covers http, embedded credentials, localhost and direct IPv4/IPv6 private/loopback hosts: PASS.

## Critical remaining blockers
- Actual VPS native GUI capture and Chromium-reference visual comparison for this revision: **not yet performed** at the time of this record; required before visual acceptance.
- Only the first twelve unique images are loaded, using source-order rather than viewport visibility. Responsive image srcset/picture selection, animated GIF playback, CSS background images, image cache persistence and many image formats remain incomplete. Intrinsic image dimensions are not fed back into layout (HTML width/height or conservative placeholder instead).
- CSS selection/cascade is incomplete: 174 selector lists and 36 declarations from CNET are omitted; external CSS vs inline style source-order interleaving and modern CSS @container features remain incomplete. Browser recovery omissions are *not* conformance passes.
- Font discovery/shaping, complete HTML5 tree building, CSS box/stacking semantics, clipping, SVG embedding, rich forms and Boa-to-DOM event bridge are not implemented or not accepted.
- DNS rebinding, full CSP, complete CORS and independently signed DDC/Frequency evidence are not established. No production release approval.

**Release gate: BLOCKED until native screenshot and interactive-browser acceptance pass.**
