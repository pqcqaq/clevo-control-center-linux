#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
RPM_ARCH="${RPM_ARCH:-$(uname -m)}"
TOP_DIR="$ROOT_DIR/dist/rpm/rpmbuild"
PAYLOAD_DIR="$ROOT_DIR/dist/rpm/payload"
OUTPUT="$ROOT_DIR/dist/$APP_ID-$VERSION-1.$RPM_ARCH.rpm"
BINARY="${RELEASE_BINARY:-target/release/$APP_ID}"

if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "rpmbuild not found" >&2
    exit 1
fi

if [[ -z "${RELEASE_BINARY:-}" ]]; then
    cargo build --release
fi
if [[ ! -x "$BINARY" ]]; then
    echo "release binary not found or not executable: $BINARY" >&2
    exit 1
fi

rm -rf "$TOP_DIR" "$PAYLOAD_DIR"
mkdir -p \
    "$TOP_DIR/BUILD" \
    "$TOP_DIR/BUILDROOT" \
    "$TOP_DIR/RPMS" \
    "$TOP_DIR/SOURCES" \
    "$TOP_DIR/SPECS" \
    "$TOP_DIR/SRPMS" \
    "$PAYLOAD_DIR/usr/bin" \
    "$PAYLOAD_DIR/usr/lib/$APP_ID/module" \
    "$PAYLOAD_DIR/usr/share/applications" \
    "$PAYLOAD_DIR/usr/share/doc/$APP_ID"

install -m 0755 "$BINARY" \
    "$PAYLOAD_DIR/usr/lib/$APP_ID/$APP_ID"
install -m 0644 module/Makefile \
    "$PAYLOAD_DIR/usr/lib/$APP_ID/module/Makefile"
install -m 0644 module/clevo_control_center.c \
    "$PAYLOAD_DIR/usr/lib/$APP_ID/module/clevo_control_center.c"

cat > "$PAYLOAD_DIR/usr/bin/$APP_ID" <<EOF
#!/usr/bin/env bash
cd /usr/lib/$APP_ID
exec /usr/lib/$APP_ID/$APP_ID "\$@"
EOF
chmod 0755 "$PAYLOAD_DIR/usr/bin/$APP_ID"

sed "s#^Exec=.*#Exec=/usr/bin/$APP_ID#" "app/$APP_ID.desktop" \
    > "$PAYLOAD_DIR/usr/share/applications/$APP_ID.desktop"
chmod 0644 "$PAYLOAD_DIR/usr/share/applications/$APP_ID.desktop"

install -m 0644 README.md "$PAYLOAD_DIR/usr/share/doc/$APP_ID/README.md"
install -m 0644 DCHU_ADJUSTMENTS.md \
    "$PAYLOAD_DIR/usr/share/doc/$APP_ID/DCHU_ADJUSTMENTS.md"
install -m 0644 CONTRIBUTING.md \
    "$PAYLOAD_DIR/usr/share/doc/$APP_ID/CONTRIBUTING.md"
install -m 0644 SECURITY.md "$PAYLOAD_DIR/usr/share/doc/$APP_ID/SECURITY.md"
install -m 0644 LICENSE "$PAYLOAD_DIR/usr/share/doc/$APP_ID/LICENSE"

tar -C "$PAYLOAD_DIR" -czf "$TOP_DIR/SOURCES/payload.tar.gz" .
sed \
    -e "s/@VERSION@/$VERSION/g" \
    -e "s/@ARCH@/$RPM_ARCH/g" \
    packaging/rpm/clevo-control-center.spec.in \
    > "$TOP_DIR/SPECS/$APP_ID.spec"

rpmbuild --define "_topdir $TOP_DIR" -bb "$TOP_DIR/SPECS/$APP_ID.spec"

built_rpm="$(find "$TOP_DIR/RPMS" -type f -name "$APP_ID-$VERSION-1*.rpm" -print -quit)"
if [[ -z "$built_rpm" ]]; then
    echo "rpmbuild completed without producing the expected package" >&2
    exit 1
fi

install -m 0644 "$built_rpm" "$OUTPUT"
echo "$OUTPUT"
