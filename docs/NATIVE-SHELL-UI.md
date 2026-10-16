# Altru Browser - native shell design contract

## Product identity
- Desktop shell: eframe/egui; HTML and CSS rendering remain inside the owned native engine.
- Official logo source: historical approved Git asset fea5ea24691490c8b970518d5dc88be9eafedf3a:assets/brand/altru-browser-logo-github.png (SHA-256 41b224769811b0870354d58368f9bc8ea45ad4192e92d76791b896c27d61654b).
- Derived window icon: assets/brand/altru-browser-approved-app.png, based on approved artwork with contour stroke, no invented motif.
- No faux-vyshyvanka decorations or unverified browser-compatibility claims.

## Interaction contract
- Navigation stays visible after successful loads. Explicit Focus mode hides toolbar.
- Back, forward, reload, home use existing NativeRuntime operations.
- Address bar and Go accept HTTPS and built-in altru://start; unsupported schemes remain denied.
- Ctrl+L focuses address; Escape enters Focus; F9 toggles Page Intelligence; Ctrl+R reloads; Alt+Left/Right navigate history.
- Theme toggle switches chrome; dark chrome is the default. The document uses native rendering.
- Status footer transparently labels developer-preview and non-promoted state.

## Verification requirements
- cargo check --offline --features desktop-preview --bin altru-browser
- cargo test --offline --features desktop-preview --bin altru-browser
- Visual rendering, navigation focus, DPI and small-window checks require real GUI review before production promotion.
- Pre-existing edits to src/native_html.rs are out of scope and must remain untouched.
