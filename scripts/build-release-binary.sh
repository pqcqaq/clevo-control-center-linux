#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="clevo-control-center"
BUILD_IMAGE="${RELEASE_BUILD_IMAGE:-rust:1.88-bullseye}"
TARGET_DIR="$ROOT_DIR/dist/release-target-bullseye"
CARGO_HOME_DIR="$ROOT_DIR/dist/release-cargo-home-bullseye"
BINARY="$TARGET_DIR/release/$APP_ID"

if ! command -v docker >/dev/null 2>&1; then
    echo "Docker is required to build the portable glibc 2.31 release binary" >&2
    exit 1
fi

mkdir -p "$TARGET_DIR" "$CARGO_HOME_DIR"

docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e HOME=/tmp/clevo-build-home \
    -e CARGO_HOME=/cargo-home \
    -e CARGO_TARGET_DIR=/workspace/dist/release-target-bullseye \
    -v "$ROOT_DIR:/workspace" \
    -v "$CARGO_HOME_DIR:/cargo-home" \
    -w /workspace \
    "$BUILD_IMAGE" \
    cargo build --release --locked

if [[ ! -x "$BINARY" ]]; then
    echo "release container did not produce $BINARY" >&2
    exit 1
fi

echo "$BINARY"
