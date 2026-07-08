#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -x target/release/clevo-keyboard-led ]]; then
    exec target/release/clevo-keyboard-led --service
fi

exec cargo run -- --service
