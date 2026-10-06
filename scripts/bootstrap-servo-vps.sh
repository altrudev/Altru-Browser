#!/usr/bin/env bash
set -euo pipefail

ROOT="${AWEF_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SYSROOT="${AWEF_SYSROOT:-$HOME/src/awef-sysroot}"
PKGS="$SYSROOT/pkgs"
LOCALBIN="$HOME/.local/bin"

mkdir -p "$PKGS" "$LOCALBIN"
ln -sf /usr/bin/llvm-objdump-21 "$LOCALBIN/llvm-objdump"

cd "$PKGS"

packages=(
  libfontconfig1-dev
  libfontconfig-dev
  libfreetype-dev
  libbz2-dev
  libpng-dev
  libbrotli-dev
  zlib1g-dev
)

for package in "${packages[@]}"; do
  apt-get download "$package"
done

for deb in ./*.deb; do
  dpkg-deb -x "$deb" "$SYSROOT"
done

pcdir="$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig"
mkdir -p "$pcdir"

if [[ ! -f "$pcdir/bzip2.pc" ]]; then
  cat > "$pcdir/bzip2.pc" <<'EOF'
prefix=/usr
exec_prefix=${prefix}
libdir=${exec_prefix}/lib/x86_64-linux-gnu
includedir=${prefix}/include

Name: bzip2
Description: bzip2 compression library
Version: 1.0.8
Libs: -L${libdir} -lbz2
Cflags: -I${includedir}
EOF
fi

export PATH="$LOCALBIN:$PATH"
export PKG_CONFIG_PATH="$pcdir"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"

pkg-config --cflags --libs fontconfig

printf 'AWEF Servo VPS overlay ready.\n'
printf 'ROOT=%s\nSYSROOT=%s\n' "$ROOT" "$SYSROOT"
