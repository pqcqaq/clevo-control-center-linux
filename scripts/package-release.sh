#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
MACHINE_ARCH="$(uname -m)"
SOURCE_ARCHIVE="$ROOT_DIR/dist/$APP_ID-$VERSION-source.tar.gz"
RUN_CHECKS=1

case "${1:-}" in
    "") ;;
    --skip-checks) RUN_CHECKS=0 ;;
    --help|-h)
        echo "usage: $0 [--skip-checks]"
        echo "  --skip-checks  build packages without rerunning fmt, Clippy, tests, or Kbuild"
        exit 0
        ;;
    *)
        echo "unknown argument: $1" >&2
        echo "usage: $0 [--skip-checks]" >&2
        exit 2
        ;;
esac

case "$MACHINE_ARCH" in
    x86_64)
        DEB_ARCH="amd64"
        RPM_ARCH="x86_64"
        ARCH_ARCH="x86_64"
        ;;
    aarch64|arm64)
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        ARCH_ARCH="aarch64"
        ;;
    *)
        echo "unsupported release architecture: $MACHINE_ARCH" >&2
        exit 1
        ;;
esac

export DEB_ARCH RPM_ARCH ARCH_ARCH

if ! git diff --quiet --ignore-submodules -- || \
   ! git diff --cached --quiet --ignore-submodules -- || \
   [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
    echo "release packaging requires no uncommitted public changes" >&2
    exit 1
fi

if [[ "$RUN_CHECKS" -eq 1 ]]; then
    echo '[check] Rust formatting'
    cargo fmt --check

    echo '[check] Rust targets and features'
    cargo check --all-targets --all-features
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-targets --all-features

    echo '[check] kernel module'
    make -B -C module W=1
else
    echo '[skip] release checks disabled by --skip-checks' >&2
fi

mkdir -p dist
rm -f \
    "$ROOT_DIR/dist/$APP_ID-$VERSION-source.tar.gz" \
    "$ROOT_DIR/dist/$APP_ID-$VERSION-linux-$MACHINE_ARCH.tar.gz" \
    "$ROOT_DIR/dist/${APP_ID}_${VERSION}_${DEB_ARCH}.deb" \
    "$ROOT_DIR/dist/$APP_ID-$VERSION-1.$RPM_ARCH.rpm" \
    "$ROOT_DIR/dist/$APP_ID-$VERSION-1-$ARCH_ARCH.pkg.tar.zst" \
    "$ROOT_DIR/dist/RELEASE_ASSETS.txt" \
    "$ROOT_DIR/dist/SHA256SUMS"

assets=()
deb_asset='[not built] Debian package'
rpm_asset='[not built] RPM package'
arch_asset='[not built] Arch Linux package'

echo '[build] portable release binary (Debian Bullseye / glibc 2.31)'
RELEASE_BINARY="$("$ROOT_DIR/scripts/build-release-binary.sh")"
export RELEASE_BINARY

echo '[package] source archive'
git archive \
    --format=tar.gz \
    --prefix="$APP_ID-$VERSION/" \
    --output="$SOURCE_ARCHIVE" \
    HEAD
assets+=("$(basename "$SOURCE_ARCHIVE")")

echo '[package] portable archive'
"$ROOT_DIR/scripts/package-tar.sh"
assets+=("$APP_ID-$VERSION-linux-$MACHINE_ARCH.tar.gz")

if command -v dpkg-deb >/dev/null 2>&1; then
    echo '[package] Debian archive'
    "$ROOT_DIR/scripts/package-deb.sh"
    deb_asset="${APP_ID}_${VERSION}_${DEB_ARCH}.deb"
    assets+=("$deb_asset")
else
    echo '[skip] dpkg-deb not found; Debian archive was not built' >&2
fi

if command -v rpmbuild >/dev/null 2>&1; then
    echo '[package] RPM archive'
    "$ROOT_DIR/scripts/package-rpm.sh"
    rpm_asset="$APP_ID-$VERSION-1.$RPM_ARCH.rpm"
    assets+=("$rpm_asset")
else
    echo '[skip] rpmbuild not found; RPM archive was not built' >&2
fi

if command -v makepkg >/dev/null 2>&1 || command -v docker >/dev/null 2>&1; then
    echo '[package] Arch Linux archive'
    "$ROOT_DIR/scripts/package-arch.sh"
    arch_asset="$APP_ID-$VERSION-1-$ARCH_ARCH.pkg.tar.zst"
    assets+=("$arch_asset")
else
    echo '[skip] makepkg and Docker not found; Arch archive was not built' >&2
fi

cat > "$ROOT_DIR/dist/RELEASE_ASSETS.txt" <<EOF
Clevo Control Center $VERSION release assets ($MACHINE_ARCH)

$APP_ID-$VERSION-source.tar.gz
  Source archive from the exact Git HEAD used for this release.

$APP_ID-$VERSION-linux-$MACHINE_ARCH.tar.gz
  Portable user-level installer for other Linux distributions.

$deb_asset
  Debian, Ubuntu, Kali, Linux Mint, and compatible distributions.

$rpm_asset
  Fedora, RHEL/Rocky/AlmaLinux 9+, and openSUSE-family distributions.

$arch_asset
  Arch Linux, Manjaro, EndeavourOS, and compatible distributions.

Install matching kernel headers on the target system. Every binary package
carries module source and builds it for the running kernel during installation.
The executable is built in Debian Bullseye and requires glibc 2.31 or newer.
EOF
assets+=("RELEASE_ASSETS.txt")

echo '[package] SHA-256 checksums'
(
    cd dist
    sha256sum "${assets[@]}" > SHA256SUMS
)

echo "release artifacts written to $ROOT_DIR/dist"
