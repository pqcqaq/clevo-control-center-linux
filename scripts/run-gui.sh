#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -x target/release/clevo-control-center ]]; then
    exec target/release/clevo-control-center "$@"
fi

exec cargo run -- "$@"
