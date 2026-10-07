#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/altru-browser"
ICON="$ROOT/assets/brand/altru-browser-mark.png"

if [[ ! -x "$BIN" ]]; then
  echo "Building Altru Browser developer preview..."
  (cd "$ROOT" && cargo build --release --features desktop-preview --bin altru-browser)
fi

INSTALL_DIR="$HOME/.local/lib/altru-browser"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$INSTALL_DIR" "$APP_DIR" "$ICON_DIR"

install -m 0755 "$BIN" "$INSTALL_DIR/altru-browser"
install -m 0644 "$ICON" "$ICON_DIR/altru-browser.png"

cat > "$APP_DIR/altru-browser.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Altru Browser
GenericName=Web Browser Developer Preview
Comment=Independent adaptive browser — Code for Humanity
Exec=$INSTALL_DIR/altru-browser
Icon=altru-browser
Terminal=false
Categories=Network;WebBrowser;Development;
StartupNotify=true
StartupWMClass=Altru Browser
Keywords=browser;web;altru;privacy;native;
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed Altru Browser developer preview."
echo "Launch it from your application menu as: Altru Browser"
