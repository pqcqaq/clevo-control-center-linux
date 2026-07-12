#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
VERSION="$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')"
SOURCE_ARCHIVE="$ROOT_DIR/dist/$APP_ID-$VERSION-source.tar.gz"

if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
    echo "release packaging requires a clean Git worktree" >&2
    exit 1
fi

echo '[check] Rust formatting'
cargo fmt --check

echo '[check] Rust targets and features'
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features

echo '[check] kernel module'
make -B -C module W=1

mkdir -p dist

echo '[package] source archive'
git archive \
    --format=tar.gz \
    --prefix="$APP_ID-$VERSION/" \
    --output="$SOURCE_ARCHIVE" \
    HEAD

echo '[package] portable archive'
"$ROOT_DIR/scripts/package-tar.sh"

if command -v dpkg-deb >/dev/null 2>&1; then
    echo '[package] Debian archive'
    "$ROOT_DIR/scripts/package-deb.sh"
else
    echo '[skip] dpkg-deb not found; Debian archive was not built' >&2
fi

echo '[package] SHA-256 checksums'
(
    cd dist
    rm -f SHA256SUMS
    find . -maxdepth 1 -type f \
        \( -name "$APP_ID-$VERSION-*.tar.gz" -o -name "${APP_ID}_${VERSION}_*.deb" \) \
        -print0 \
        | sort -z \
        | xargs -0 sha256sum \
        | sed 's#  \./#  #' \
        > SHA256SUMS
)

echo "release artifacts written to $ROOT_DIR/dist"
