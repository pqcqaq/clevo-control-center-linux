#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

"$ROOT_DIR/scripts/check-env.sh"

echo '[build] kernel module'
make -C module

echo '[build] Rust GUI/service'
cargo build --release

echo '[done] target/release/clevo-keyboard-led'
