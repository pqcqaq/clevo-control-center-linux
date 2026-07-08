#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-keyboard-led"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
ARCH="$(uname -m)"
STAGE="$ROOT_DIR/dist/$APP_ID-$VERSION-linux-$ARCH"
ARCHIVE="$ROOT_DIR/dist/$APP_ID-$VERSION-linux-$ARCH.tar.gz"

cargo build --release

rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/app" "$STAGE/packaging" "$STAGE/module"

install -m 0755 "target/release/$APP_ID" "$STAGE/bin/$APP_ID"
install -m 0644 "app/$APP_ID.desktop" "$STAGE/app/$APP_ID.desktop"
install -m 0755 "packaging/install.sh" "$STAGE/install.sh"
install -m 0644 README.md "$STAGE/README.md"
cp -R module/. "$STAGE/module/"
find "$STAGE/module" -type f \( -name '*.ko' -o -name '*.o' -o -name '*.mod' -o -name '*.mod.c' -o -name 'Module.symvers' -o -name 'modules.order' -o -name '.*.cmd' \) -delete
rm -rf "$STAGE/module/.tmp_versions"

tar -C "$ROOT_DIR/dist" -czf "$ARCHIVE" "$(basename "$STAGE")"
echo "$ARCHIVE"
