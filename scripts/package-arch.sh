#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
ARCH="${ARCH_ARCH:-$(uname -m)}"
BUILD_DIR="$ROOT_DIR/dist/arch/build"
OUTPUT="$ROOT_DIR/dist/$APP_ID-$VERSION-1-$ARCH.pkg.tar.zst"
ARCH_BUILD_IMAGE="${ARCH_BUILD_IMAGE:-archlinux:base-devel}"
BINARY="${RELEASE_BINARY:-target/release/$APP_ID}"
PACKAGER="Clevo Control Center Contributors <qcqcqc@zust.online>"

if [[ -z "${RELEASE_BINARY:-}" ]]; then
    cargo build --release
fi
if [[ ! -x "$BINARY" ]]; then
    echo "release binary not found or not executable: $BINARY" >&2
    exit 1
fi

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

install -m 0755 "$BINARY" "$BUILD_DIR/$APP_ID"
install -m 0644 "app/$APP_ID.desktop" "$BUILD_DIR/$APP_ID.desktop"
install -m 0644 module/Makefile "$BUILD_DIR/module.Makefile"
install -m 0644 module/clevo_control_center.c \
    "$BUILD_DIR/clevo_control_center.c"
install -m 0644 README.md "$BUILD_DIR/README.md"
install -m 0644 DCHU_ADJUSTMENTS.md "$BUILD_DIR/DCHU_ADJUSTMENTS.md"
install -m 0644 CONTRIBUTING.md "$BUILD_DIR/CONTRIBUTING.md"
install -m 0644 SECURITY.md "$BUILD_DIR/SECURITY.md"
install -m 0644 LICENSE "$BUILD_DIR/LICENSE"
install -m 0644 packaging/arch/clevo-control-center.install \
    "$BUILD_DIR/clevo-control-center.install"
sed \
    -e "s/@VERSION@/$VERSION/g" \
    -e "s/@ARCH@/$ARCH/g" \
    packaging/arch/PKGBUILD.in \
    > "$BUILD_DIR/PKGBUILD"

if command -v makepkg >/dev/null 2>&1; then
    (
        cd "$BUILD_DIR"
        PACKAGER="$PACKAGER" makepkg --force --nodeps --noconfirm
    )
elif command -v docker >/dev/null 2>&1; then
    docker run --rm \
        -e BUILD_UID="$(id -u)" \
        -e BUILD_GID="$(id -g)" \
        -e CLEVO_PACKAGER="$PACKAGER" \
        -v "$BUILD_DIR:/build" \
        "$ARCH_BUILD_IMAGE" \
        bash -lc '
            groupadd -g "$BUILD_GID" builder 2>/dev/null || true
            useradd -m -u "$BUILD_UID" -g "$BUILD_GID" builder
            chown -R "$BUILD_UID:$BUILD_GID" /build
            sed -i "s|^#\?PACKAGER=.*|PACKAGER=\"$CLEVO_PACKAGER\"|" /etc/makepkg.conf
            runuser -u builder -- bash -lc "cd /build && makepkg --force --nodeps --noconfirm"
        '
else
    echo "makepkg not found and Docker is unavailable" >&2
    exit 1
fi

built_package="$BUILD_DIR/$APP_ID-$VERSION-1-$ARCH.pkg.tar.zst"
if [[ ! -f "$built_package" ]]; then
    echo "makepkg completed without producing $built_package" >&2
    exit 1
fi

install -m 0644 "$built_package" "$OUTPUT"
echo "$OUTPUT"
