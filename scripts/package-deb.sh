#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
LEGACY_APP_ID="clevo-keyboard-led"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
DEB_ARCH="${DEB_ARCH:-amd64}"
BUILD_DIR="$ROOT_DIR/dist/deb/$APP_ID"
OUTPUT="$ROOT_DIR/dist/${APP_ID}_${VERSION}_${DEB_ARCH}.deb"

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "dpkg-deb not found" >&2
    exit 1
fi

cargo build --release

rm -rf "$BUILD_DIR"
mkdir -p \
    "$BUILD_DIR/DEBIAN" \
    "$BUILD_DIR/usr/bin" \
    "$BUILD_DIR/usr/lib/$APP_ID/module" \
    "$BUILD_DIR/usr/share/applications" \
    "$BUILD_DIR/usr/share/doc/$APP_ID"

install -m 0755 "target/release/$APP_ID" "$BUILD_DIR/usr/lib/$APP_ID/$APP_ID"
cp -R module/. "$BUILD_DIR/usr/lib/$APP_ID/module/"
find "$BUILD_DIR/usr/lib/$APP_ID/module" -type f \( -name '*.ko' -o -name '*.o' -o -name '*.mod' -o -name '*.mod.c' -o -name 'Module.symvers' -o -name 'modules.order' -o -name '.*.cmd' \) -delete
rm -rf "$BUILD_DIR/usr/lib/$APP_ID/module/.tmp_versions"

cat > "$BUILD_DIR/usr/bin/$APP_ID" <<EOF
#!/usr/bin/env bash
cd /usr/lib/$APP_ID
exec /usr/lib/$APP_ID/$APP_ID "\$@"
EOF
chmod 0755 "$BUILD_DIR/usr/bin/$APP_ID"

cat > "$BUILD_DIR/usr/bin/$LEGACY_APP_ID" <<EOF
#!/usr/bin/env bash
exec /usr/bin/$APP_ID "\$@"
EOF
chmod 0755 "$BUILD_DIR/usr/bin/$LEGACY_APP_ID"

sed "s#^Exec=.*#Exec=/usr/bin/$APP_ID#" "app/$APP_ID.desktop" > "$BUILD_DIR/usr/share/applications/$APP_ID.desktop"
chmod 0644 "$BUILD_DIR/usr/share/applications/$APP_ID.desktop"
install -m 0644 README.md "$BUILD_DIR/usr/share/doc/$APP_ID/README.md"
install -m 0644 packaging/deb/control "$BUILD_DIR/DEBIAN/control"
sed -i "s/^Version: .*/Version: $VERSION/" "$BUILD_DIR/DEBIAN/control"
sed -i "s/^Architecture: .*/Architecture: $DEB_ARCH/" "$BUILD_DIR/DEBIAN/control"
install -m 0755 packaging/deb/postinst "$BUILD_DIR/DEBIAN/postinst"
install -m 0755 packaging/deb/postrm "$BUILD_DIR/DEBIAN/postrm"

dpkg-deb --root-owner-group --build "$BUILD_DIR" "$OUTPUT"
echo "$OUTPUT"
