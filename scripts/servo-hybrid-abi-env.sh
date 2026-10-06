#!/usr/bin/env bash
set -euo pipefail

SYSROOT="${AWEF_SYSROOT:-$HOME/src/awef-sysroot}"
OVERLAY_PC="$SYSROOT/hybrid-pkgconfig"
HOST_LIBDIR="/usr/lib/x86_64-linux-gnu"
SOURCE_PC="$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig"

mkdir -p "$OVERLAY_PC"

fontconfig_source="$SOURCE_PC/fontconfig.pc"
freetype_source="$SOURCE_PC/freetype2.pc"

test -f "$fontconfig_source"
test -f "$freetype_source"

fontconfig_version="$(awk -F': ' '/^Version:/ {print $2; exit}' "$fontconfig_source")"
freetype_version="$(awk -F': ' '/^Version:/ {print $2; exit}' "$freetype_source")"

fontconfig_runtime="$(dpkg-query -W -f='${Version}' libfontconfig1)"
freetype_runtime="$(dpkg-query -W -f='${Version}' libfreetype6)"

fontconfig_dev_deb="$(ls -1t "$SYSROOT"/pkgs/libfontconfig-dev_*.deb | head -1)"
freetype_dev_deb="$(ls -1t "$SYSROOT"/pkgs/libfreetype-dev_*.deb | head -1)"

fontconfig_dev="$(dpkg-deb -f "$fontconfig_dev_deb" Version)"
freetype_dev="$(dpkg-deb -f "$freetype_dev_deb" Version)"

if [[ "$fontconfig_dev" != "$fontconfig_runtime" ]]; then
    printf 'Fontconfig ABI mismatch: dev=%s runtime=%s\n' "$fontconfig_dev" "$fontconfig_runtime" >&2
    exit 1
fi

if [[ "$freetype_dev" != "$freetype_runtime" ]]; then
    printf 'FreeType ABI mismatch: dev=%s runtime=%s\n' "$freetype_dev" "$freetype_runtime" >&2
    exit 1
fi

cat > "$OVERLAY_PC/freetype2.pc" <<EOF
prefix=$SYSROOT/usr
exec_prefix=\${prefix}
libdir=$HOST_LIBDIR
includedir=\${prefix}/include

Name: FreeType 2
Description: AWEF hybrid development overlay
Version: $freetype_version
Libs: -L\${libdir} -lfreetype
Cflags: -I\${includedir}/freetype2
EOF

cat > "$OVERLAY_PC/fontconfig.pc" <<EOF
prefix=$SYSROOT/usr
exec_prefix=\${prefix}
libdir=$HOST_LIBDIR
includedir=\${prefix}/include

Name: Fontconfig
Description: AWEF hybrid development overlay
Version: $fontconfig_version
Requires: freetype2
Libs: -L\${libdir} -lfontconfig
Cflags: -I\${includedir}
EOF

export PKG_CONFIG_PATH="$OVERLAY_PC:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
unset PKG_CONFIG_SYSROOT_DIR || true
export FONTCONFIG_DYNAMIC=1
export FREETYPE2_DYNAMIC=1

flags="$(pkg-config --libs fontconfig)"
case "$flags" in
    *"$SYSROOT/usr/lib"*)
        printf 'Hybrid ABI invariant failed: private library directory still selected: %s\n' "$flags" >&2
        exit 1
        ;;
esac

printf 'AWEF hybrid ABI ready: fontconfig=%s freetype=%s flags=%s\n'     "$fontconfig_runtime" "$freetype_runtime" "$flags"
